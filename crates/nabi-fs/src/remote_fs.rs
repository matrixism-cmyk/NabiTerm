//! 원격 파일시스템 트레잇 + DTO.

use async_trait::async_trait;

/// 파일 종류.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
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
