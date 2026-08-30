//! SFTP 재귀 디렉터리 전송·검색 — download_dir/upload_dir/search. fs.rs(단일 파일 스트리밍)에서 분리.
//! 공개 RemoteFs 메서드만 사용하므로 SftpFs의 비공개 필드에 의존하지 않는다.

use crate::fs::SftpFs;
use nabi_fs::{FileKind, RemoteFs};

/// 폴더 전송 진행 상황 — 지금까지 **끝난 파일들의 합**과 보고 콜백.
///
/// 폴더 전송은 파일 하나짜리 전송과 달리 진행률을 아무도 보고하지 않았다. 큐에는 줄만
/// 생기고 끝날 때까지 한 번도 움직이지 않아, 큰 폴더에서는 멈춘 것과 구별되지 않았다.
pub struct DirProgress<'c> {
    /// 완료된 파일들의 누적 바이트(진행 중 파일의 부분 진행은 여기에 더해 보고한다).
    pub done: u64,
    /// 건너뛴 것(링크 등) 개수 — 조용히 빠뜨리면 복사가 덜 된 줄 모른다.
    pub skipped: usize,
    pub cb: &'c mut (dyn FnMut(u64) + Send),
}

impl DirProgress<'_> {
    /// 진행 중 파일의 부분 바이트를 얹어 보고한다.
    pub(crate) fn report(&mut self, partial: u64) {
        let total = self.done + partial;
        (self.cb)(total);
    }
}

impl SftpFs {
    /// 원격 디렉터리를 로컬로 재귀 다운로드한다(하위 폴더·파일 전부).
    pub async fn download_dir(&mut self, remote: &str, local: &std::path::Path) -> Result<(), String> {
        let mut noop = |_: u64| {};
        let mut p = DirProgress { done: 0, skipped: 0, cb: &mut noop };
        self.download_dir_progress(remote, local, &mut p).await
    }

    /// 진행률을 보고하며 재귀 다운로드한다.
    pub fn download_dir_progress<'a>(
        &'a mut self,
        remote: &'a str,
        local: &'a std::path::Path,
        prog: &'a mut DirProgress<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            std::fs::create_dir_all(local).map_err(|e| e.to_string())?;
            for e in self.list_dir(remote).await? {
                // 이름은 **서버가 준 것**이다. 그대로 이어 붙이면 받는 폴더를 벗어난다
                // (`..\evil` 은 위로, `C:\...` 는 폴더를 통째로 갈아 치운다).
                // 건너뛴 개수를 세어 둔다 — 파일이 하나 없는데 아무 말도 없으면 받은 줄 안다.
                if !crate::safename::is_safe_entry_name(&e.name) {
                    prog.skipped += 1;
                    continue;
                }
                let rpath = format!("{}/{}", remote.trim_end_matches('/'), e.name);
                let lpath = local.join(&e.name);
                if matches!(e.kind, FileKind::Dir) {
                    self.download_dir_progress(&rpath, &lpath, prog).await?;
                } else {
                    // 스트리밍 다운로드(통째 메모리 적재 대신 청크 + 원자적 .filepart + 크기검증).
                    let lp = lpath.to_string_lossy();
                    self.download(&rpath, lp.as_ref(), 0, |b| prog.report(b)).await?;
                    prog.done += e.size; // 이 파일은 끝났다 — 다음 파일의 기준점이 된다.
                    prog.report(0);
                }
            }
            Ok(())
        })
    }

    /// 로컬 디렉터리를 원격으로 재귀 업로드한다(하위 폴더·파일 전부, 원격 폴더 생성).
    pub async fn upload_dir(&mut self, local: &std::path::Path, remote: &str) -> Result<(), String> {
        let mut noop = |_: u64| {};
        let mut p = DirProgress { done: 0, skipped: 0, cb: &mut noop };
        self.upload_dir_progress(local, remote, &mut p).await
    }

    /// 진행률을 보고하며 재귀 업로드한다.
    pub fn upload_dir_progress<'a>(
        &'a mut self,
        local: &'a std::path::Path,
        remote: &'a str,
        prog: &'a mut DirProgress<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            let _ = self.mkdir(remote).await; // 있으면 무시.
            for entry in std::fs::read_dir(local).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let rpath = format!("{}/{}", remote.trim_end_matches('/'), name);
                let lpath = entry.path();
                // **링크는 따라가지 않는다.** `is_dir()` 는 링크를 따라가므로, 위를 가리키는
                // 링크가 하나 있으면 끝없이 돈다(`a -> ..`). 그리고 폴더 밖을 가리키는
                // 링크를 따라가면 사용자가 고르지 않은 파일이 서버로 올라간다.
                //
                // 내려받는 쪽은 링크를 종류로 구분해 이미 안 따라간다(`FileKind::Symlink`).
                // 올리는 쪽만 빠져 있었다.
                let link = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
                if link {
                    prog.skipped += 1;
                    continue;
                }
                if lpath.is_dir() {
                    self.upload_dir_progress(&lpath, &rpath, prog).await?;
                } else {
                    // 스트리밍 업로드(통째 메모리 적재 대신 청크 + 원자적 교체 + 크기검증).
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let lp = lpath.to_string_lossy();
                    self.upload(lp.as_ref(), &rpath, |b| prog.report(b)).await?;
                    prog.done += size;
                    prog.report(0);
                }
            }
            Ok(())
        })
    }

    /// 권한을 재귀 적용한다(대상 + 디렉터리면 하위 전부). 파일이면 대상만.
    pub fn chmod_recursive<'a>(
        &'a mut self,
        path: &'a str,
        mode: u32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            self.chmod(path, mode).await?;
            if let Ok(entries) = self.list_dir(path).await {
                for e in entries {
                    if e.name == "." || e.name == ".." {
                        continue;
                    }
                    let child = format!("{}/{}", path.trim_end_matches('/'), e.name);
                    if matches!(e.kind, FileKind::Dir) {
                        self.chmod_recursive(&child, mode).await?;
                    } else {
                        self.chmod(&child, mode).await?;
                    }
                }
            }
            Ok(())
        })
    }

    /// 원격 경로를 재귀 삭제한다(디렉터리면 내용 전부 삭제 후 폴더 제거, 파일이면 바로 삭제).
    pub fn remove_recursive<'a>(
        &'a mut self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            match self.list_dir(path).await {
                Ok(entries) => {
                    for e in entries {
                        if e.name == "." || e.name == ".." {
                            continue;
                        }
                        let child = format!("{}/{}", path.trim_end_matches('/'), e.name);
                        if matches!(e.kind, FileKind::Dir) {
                            self.remove_recursive(&child).await?;
                        } else {
                            self.remove(&child).await?;
                        }
                    }
                    self.remove(path).await // 비워진 디렉터리 제거.
                }
                // 목록 실패 = 디렉터리 아님(파일/링크) → 그대로 삭제.
                Err(_) => self.remove(path).await,
            }
        })
    }

    /// 원격 디렉터리를 재귀 집계한다 → (파일 수, 폴더 수, 총 바이트). 오류 시 (0,0,0).
    pub fn dir_stats<'a>(
        &'a mut self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = (u64, u64, u64)> + Send + 'a>> {
        Box::pin(async move {
            let (mut files, mut dirs, mut bytes) = (0u64, 0u64, 0u64);
            let Ok(entries) = self.list_dir(path).await else {
                return (0, 0, 0);
            };
            for e in entries {
                if e.name == "." || e.name == ".." {
                    continue;
                }
                let child = format!("{}/{}", path.trim_end_matches('/'), e.name);
                if matches!(e.kind, FileKind::Dir) {
                    dirs += 1;
                    let (f, d, b) = self.dir_stats(&child).await;
                    files += f;
                    dirs += d;
                    bytes += b;
                } else {
                    files += 1;
                    bytes += e.size;
                }
            }
            (files, dirs, bytes)
        })
    }

    /// root 아래 **파일** 트리를 (상대경로, 크기, mtime)으로 수집(동기화 계획용) —
    /// 백엔드 공용 [`nabi_fs::walk_tree`] 위임(FTP와 로직 공유, DRY).
    pub async fn list_tree(
        &mut self,
        root: &str,
        prefix: &str,
        out: &mut Vec<(String, u64, u64)>,
    ) -> Result<(), String> {
        nabi_fs::walk_tree(self, root, prefix, out).await
    }

    /// root 아래를 재귀 검색해 이름이 `needle` 에 맞는 경로들을 모은다(최대 max).
    ///
    /// 맞추기는 [`crate::namematch::matches`] 를 쓴다 — 글로브(`*.conf`)도 통한다. 예전에는
    /// 여기서만 `contains` 를 써서 찾기 창과 답이 달랐다(배치 AD).
    pub fn search<'a>(
        &'a mut self,
        root: &'a str,
        needle: &'a str,
        max: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut out = Vec::new();
            let Ok(entries) = self.list_dir(root).await else {
                return out;
            };
            for e in entries {
                if out.len() >= max {
                    break;
                }
                if e.name == "." || e.name == ".." {
                    continue;
                }
                let path = format!("{}/{}", root.trim_end_matches('/'), e.name);
                // 규칙은 `namematch` 한 곳에만 둔다 — 찾기 창과 같은 답을 내야 한다.
                if crate::namematch::matches(&e.name, needle) {
                    out.push(path.clone());
                }
                if matches!(e.kind, FileKind::Dir) {
                    out.append(&mut self.search(&path, needle, max).await);
                }
            }
            out
        })
    }
}

#[cfg(test)]
mod 링크는_따라가지_않는다 {
    /// 올릴 때 링크를 걸러 내는 판단만 따로 본다(진짜 업로드는 실서버 시험이 맡는다).
    ///
    /// `is_dir()` 는 링크를 따라간다. 위를 가리키는 링크가 하나 있으면 끝없이 돌고,
    /// 폴더 밖을 가리키는 링크를 따라가면 **사용자가 고르지 않은 파일이 올라간다.**
    /// 그래서 `file_type().is_symlink()` 로 먼저 거른다.
    ///
    /// 윈도우에서 링크 만들기는 권한이 필요할 수 있다 — 못 만들면 시험을 건너뛴다
    /// (권한 없는 PC 에서 빨개지면 아무도 안 보게 된다).
    #[test]
    fn 링크는_폴더로_보지_않는다() {
        let base = std::env::temp_dir().join(format!("nabi-linktest-{}", std::process::id()));
        let real = base.join("real");
        let _ = std::fs::create_dir_all(&real);
        let link = base.join("loop");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&base, &link).is_ok();
        #[cfg(not(windows))]
        let made = std::os::unix::fs::symlink(&base, &link).is_ok();
        if !made {
            let _ = std::fs::remove_dir_all(&base);
            return; // 링크를 못 만드는 환경 — 판단할 것이 없다.
        }
        let e = std::fs::read_dir(&base)
            .unwrap()
            .flatten()
            .find(|e| e.file_name() == "loop")
            .expect("링크를 못 찾았다");
        assert!(e.file_type().unwrap().is_symlink(), "링크로 안 보인다");
        // `is_dir()` 은 따라가므로 참이다 — 그래서 그것만 보면 안 된다는 것이 요지다.
        assert!(e.path().is_dir(), "is_dir 는 링크를 따라간다(그래서 위험하다)");
        let _ = std::fs::remove_dir_all(&base);
    }
}
