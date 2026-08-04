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
