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
    pub cb: &'c mut (dyn FnMut(u64) + Send),
}

impl DirProgress<'_> {
    /// 진행 중 파일의 부분 바이트를 얹어 보고한다.
    fn report(&mut self, partial: u64) {
        let total = self.done + partial;
        (self.cb)(total);
    }
}

impl SftpFs {
    /// 원격 디렉터리를 로컬로 재귀 다운로드한다(하위 폴더·파일 전부).
    pub async fn download_dir(&mut self, remote: &str, local: &std::path::Path) -> Result<(), String> {
        let mut noop = |_: u64| {};
        let mut p = DirProgress { done: 0, cb: &mut noop };
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
                if e.name == "." || e.name == ".." {
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
        let mut p = DirProgress { done: 0, cb: &mut noop };
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

    /// root 아래를 재귀 검색해 이름에 needle(소문자)을 포함하는 경로들을 모은다(최대 max).
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
                if e.name.to_lowercase().contains(needle) {
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
