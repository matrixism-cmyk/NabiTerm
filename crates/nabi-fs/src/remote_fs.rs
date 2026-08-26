//! 원격 파일시스템 트레잇 + DTO.

use async_trait::async_trait;

/// 파일 종류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    /// 폴더를 가리키는 심볼릭 링크.
    ///
    /// `Dir`로 합치지 않는 이유: 링크라는 사실은 사용자에게 의미가 있다(지우면 무엇이
    /// 지워지는지, 권한이 어디에 붙는지가 다르다). 그래서 **들어갈 수는 있되 링크로 보인다.**
    LinkDir,
    Other,
}

/// 디렉터리 항목.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    /// POSIX 권한 비트(rwxrwxrwx). 0 = 알 수 없음.
    pub mode: u32,
    /// 수정 시각(unix 초). 0 = 알 수 없음.
    pub mtime: u64,
}

/// 백엔드 무관 비동기 파일시스템. `Box<dyn RemoteFs>`로 쓰이므로 async_trait를 사용한다.
#[async_trait]
pub trait RemoteFs: Send {
    async fn list_dir(&mut self, path: &str) -> Result<Vec<FileEntry>, String>;
    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String>;
    async fn remove(&mut self, path: &str) -> Result<(), String>;
    async fn rename(&mut self, from: &str, to: &str) -> Result<(), String>;
    async fn mkdir(&mut self, path: &str) -> Result<(), String>;
    /// 권한 변경(POSIX mode). 기본 미지원 — SFTP만 구현.
    async fn chmod(&mut self, _path: &str, _mode: u32) -> Result<(), String> {
        Err("권한 변경 미지원".into())
    }
    /// **서버 안에서 파일 복사.** 기본 미지원 — SFTP만 구현한다.
    ///
    /// 지금까지 서버 안에서 파일을 복사하려면 받았다가 다시 올려야 했다. 그러면 회선을
    /// 두 번 타고, 큰 파일에서는 그 사이 디스크도 한 벌 쓴다. 여기서는 **디스크를 거치지
    /// 않고** 서버에서 읽어 서버로 바로 쓴다.
    ///
    /// 돌려주는 값은 복사한 바이트 수. `tick`은 누적 바이트로 진행률을 알린다.
    async fn copy_remote(
        &mut self,
        _from: &str,
        _to: &str,
        _tick: &mut (dyn FnMut(u64) + Send),
    ) -> Result<u64, String> {
        Err("서버 안 복사 미지원".into())
    }
}

/// 백엔드 무관 파일 트리 수집(동기화 계획용) — root 아래 파일을 (상대경로, 크기, mtime)으로.
/// SFTP·FTP가 공유한다(list_dir만 요구). 디렉터리 자체는 넣지 않는다(빈 폴더 v1 미지원).
pub fn walk_tree<'a, F: RemoteFs + ?Sized>(
    fs: &'a mut F,
    root: &'a str,
    prefix: &'a str,
    out: &'a mut Vec<(String, u64, u64)>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        for e in fs.list_dir(root).await? {
            if e.name == "." || e.name == ".." {
                continue;
            }
            let child = format!("{}/{}", root.trim_end_matches('/'), e.name);
            let rel = if prefix.is_empty() { e.name.clone() } else { format!("{prefix}/{}", e.name) };
            if matches!(e.kind, FileKind::Dir) {
                walk_tree(fs, &child, &rel, out).await?;
            } else {
                out.push((rel, e.size, e.mtime));
            }
        }
        Ok(())
    })
}
