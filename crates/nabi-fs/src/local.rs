//! 로컬 파일시스템 백엔드(파일 브라우저의 로컬 페인).

use crate::remote_fs::{FileEntry, FileKind, RemoteFs};
use async_trait::async_trait;
use std::path::Path;

/// std::fs 기반 로컬 백엔드.
pub struct LocalFs;

#[async_trait]
impl RemoteFs for LocalFs {
    async fn list_dir(&mut self, path: &str) -> Result<Vec<FileEntry>, String> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(path).map_err(|e| e.to_string())?.flatten() {
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let kind = if meta.is_dir() {
                FileKind::Dir
            } else if meta.is_file() {
                FileKind::File
            } else {
                FileKind::Other
            };
            out.push(FileEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                kind,
                size: meta.len(),
                mode: 0, // 로컬(Windows)은 POSIX 권한 없음.
                // 윈도우에는 uid/gid 라는 개념이 없다(SID 는 숫자 하나가 아니다).
                uid: None,
                gid: None,
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
        }
        Ok(out)
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(path).map_err(|e| e.to_string())
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
        std::fs::write(path, data).map_err(|e| e.to_string())
    }

    async fn remove(&mut self, path: &str) -> Result<(), String> {
        let p = Path::new(path);
        let r = if p.is_dir() {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        r.map_err(|e| e.to_string())
    }

    async fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        std::fs::rename(from, to).map_err(|e| e.to_string())
    }

    async fn mkdir(&mut self, path: &str) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_write_list_read_remove() {
        let dir = std::env::temp_dir().join(format!("nabi-fs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.txt");
        let fpath = file.to_str().unwrap();
        let mut fs = LocalFs;

        fs.write_file(fpath, b"hi").await.unwrap();
        let entries = fs.list_dir(dir.to_str().unwrap()).await.unwrap();
        assert!(entries
            .iter()
            .any(|e| e.name == "hello.txt" && e.kind == FileKind::File));
        assert_eq!(fs.read_file(fpath).await.unwrap(), b"hi");
        fs.remove(fpath).await.unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }
}
