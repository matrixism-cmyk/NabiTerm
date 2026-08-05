//! SFTP/FTP 공통 연결 래퍼(Conn) — sftp.rs에서 분리.
//! 공통 RemoteFs 동작은 양쪽에 위임하고, 스트리밍/재귀/검색은 SFTP에만 구현(FTP는 폴백).

use nabi_fs::{FileEntry, RemoteFs};
use std::path::Path;

/// SFTP 또는 FTP 연결(공통 RemoteFs 동작 + SFTP는 스트리밍 전송).
pub(crate) enum Conn {
    Sftp(nabi_sftp::SftpFs),
    Ftp(nabi_ftp::FtpFs),
}

impl Conn {
    /// SFTP 연결인지(재접속·이어받기 재시도는 SFTP에만 적용).
    pub(crate) fn is_sftp(&self) -> bool {
        matches!(self, Conn::Sftp(_))
    }

    /// 취소 플래그를 갈아 끼운다(워커가 작업마다 새 플래그를 쓴다). FTP는 해당 없음.
    pub(crate) fn set_cancel(&mut self, c: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        if let Conn::Sftp(f) = self {
            f.set_cancel(c);
        }
    }

    pub(crate) async fn list_dir(&mut self, p: &str) -> Result<Vec<FileEntry>, String> {
        match self {
            Conn::Sftp(f) => f.list_dir(p).await,
            Conn::Ftp(f) => f.list_dir(p).await,
        }
    }
    pub(crate) async fn rename(&mut self, a: &str, b: &str) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.rename(a, b).await,
            Conn::Ftp(f) => f.rename(a, b).await,
        }
    }
    pub(crate) async fn mkdir(&mut self, p: &str) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.mkdir(p).await,
            Conn::Ftp(f) => f.mkdir(p).await,
        }
    }
    pub(crate) async fn download(
        &mut self,
        remote: &str,
        local: &str,
        resume: u64,
        mut p: impl FnMut(u64),
    ) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.download(remote, local, resume, p).await,
            Conn::Ftp(f) => {
                let data = f.read_file(remote).await?;
                std::fs::write(local, &data).map_err(|e| e.to_string())?;
                p(data.len() as u64);
                Ok(())
            }
        }
    }
    pub(crate) async fn upload(
        &mut self,
        local: &str,
        remote: &str,
        mut p: impl FnMut(u64),
    ) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.upload(local, remote, p).await,
            Conn::Ftp(f) => {
                let data = std::fs::read(local).map_err(|e| e.to_string())?;
                f.write_file(remote, &data).await?;
                p(data.len() as u64);
                Ok(())
            }
        }
    }
    /// 폴더 재귀 다운로드. `p`는 누적 바이트를 받는다(큐 진행 막대용).
    pub(crate) async fn download_dir(
        &mut self,
        remote: &str,
        local: &Path,
        p: &mut (dyn FnMut(u64) + Send),
    ) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => {
                let mut prog = nabi_sftp::DirProgress { done: 0, cb: p };
                f.download_dir_progress(remote, local, &mut prog).await
            }
            Conn::Ftp(_) => Err("FTP: 폴더 재귀 다운로드 미지원".to_string()),
        }
    }
    /// 폴더 재귀 업로드. `p`는 누적 바이트를 받는다(큐 진행 막대용).
    pub(crate) async fn upload_dir(
        &mut self,
        local: &Path,
        remote: &str,
        p: &mut (dyn FnMut(u64) + Send),
    ) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => {
                let mut prog = nabi_sftp::DirProgress { done: 0, cb: p };
                f.upload_dir_progress(local, remote, &mut prog).await
            }
            Conn::Ftp(_) => Err("FTP: 폴더 재귀 업로드 미지원".to_string()),
        }
    }
    pub(crate) async fn chmod(&mut self, path: &str, mode: u32) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.chmod(path, mode).await,
            Conn::Ftp(f) => f.chmod(path, mode).await,
        }
    }
    pub(crate) async fn search(&mut self, root: &str, needle: &str, max: usize) -> Vec<String> {
        match self {
            Conn::Sftp(f) => f.search(root, needle, max).await,
            Conn::Ftp(_) => Vec::new(),
        }
    }
    pub(crate) async fn dir_stats(&mut self, path: &str) -> (u64, u64, u64) {
        match self {
            Conn::Sftp(f) => f.dir_stats(path).await,
            Conn::Ftp(_) => (0, 0, 0),
        }
    }
    pub(crate) async fn remove_recursive(&mut self, path: &str) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.remove_recursive(path).await,
            Conn::Ftp(f) => f.remove(path).await, // FTP는 단일 삭제(재귀 미지원).
        }
    }
    pub(crate) async fn chmod_recursive(&mut self, path: &str, mode: u32) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.chmod_recursive(path, mode).await,
            Conn::Ftp(f) => f.chmod(path, mode).await, // FTP는 단일 적용.
        }
    }
    pub(crate) async fn touch(&mut self, path: &str) -> Result<(), String> {
        match self {
            Conn::Sftp(f) => f.write_file(path, b"").await,
            Conn::Ftp(f) => f.write_file(path, b"").await,
        }
    }
}
