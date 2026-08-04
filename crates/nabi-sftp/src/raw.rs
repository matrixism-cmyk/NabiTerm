//! `RawSftpSession` 위의 얇은 계층 — 서버 확장 감지 + 우리가 쓰는 연산만 노출.
//!
//! 고수준 `SftpSession`을 쓰지 않는 이유: 내부 raw 세션을 감춰 `extended()`에 닿을 수 없다.
//! posix-rename·fsync 같은 확장과 **요청 파이프라이닝**은 raw가 있어야 가능하다.
//! 모든 메서드가 `&self`라서 요청 여러 개를 동시에 띄울 수 있다(응답은 요청 id로 다중화).

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use std::sync::Arc;

/// 대상이 이미 있어도 원자적으로 바꿔치기한다(POSIX rename 의미).
pub(crate) const POSIX_RENAME: &str = "posix-rename@openssh.com";

/// limits 확장이 없을 때 쓰는 보수적 청크. 사실상 모든 SFTP 서버가 받아들이는 크기다.
const SAFE_CHUNK: usize = 64 * 1024;
/// 서버가 더 허용해도 이 이상은 쓰지 않는다(메모리 사용과 지연의 균형).
const MAX_CHUNK: usize = 256 * 1024;

/// 서버가 VERSION/limits로 알려준 확장과 한도.
#[derive(Clone, Copy, Debug, Default)]
pub struct Feat {
    pub posix_rename: bool,
    pub fsync: bool,
    pub statvfs: bool,
    /// 한 번에 주고받을 수 있는 최대 바이트(limits@openssh.com).
    pub read_len: Option<u64>,
    pub write_len: Option<u64>,
}

/// 서버 한도를 우리 상·하한으로 자른 실제 청크 크기.
fn cap(limit: Option<u64>) -> usize {
    limit.map(|l| l.min(MAX_CHUNK as u64) as usize).unwrap_or(SAFE_CHUNK).max(4096)
}

/// SFTP 문자열 두 개를 확장 요청 페이로드로 직렬화한다(길이 u32 BE + 바이트).
pub(crate) fn ext_two_strings(a: &str, b: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + a.len() + b.len());
    for s in [a, b] {
        v.extend_from_slice(&(s.len() as u32).to_be_bytes());
        v.extend_from_slice(s.as_bytes());
    }
    v
}

fn es(e: SftpError) -> String {
    e.to_string()
}

/// raw 세션 + 감지된 확장. `Clone`은 Arc 복제라 저렴하다(전송 루프가 self를 빌리지 않게).
#[derive(Clone)]
pub struct RawFs {
    session: Arc<RawSftpSession>,
    feat: Feat,
    /// 지금까지 보낸 읽기 요청 수 — 파이프라인이 요청을 낭비하지 않는지 검증·진단용.
    reads: Arc<std::sync::atomic::AtomicU64>,
}

impl RawFs {
    pub(crate) fn new(session: RawSftpSession, feat: Feat) -> Self {
        Self { session: Arc::new(session), feat, reads: Arc::default() }
    }

    pub fn feat(&self) -> Feat {
        self.feat
    }

    /// 지금까지 보낸 읽기 요청 수(누적).
    pub fn read_requests(&self) -> u64 {
        self.reads.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn read_chunk(&self) -> usize {
        cap(self.feat.read_len)
    }

    pub fn write_chunk(&self) -> usize {
        cap(self.feat.write_len)
    }

    pub async fn open(&self, path: &str, flags: OpenFlags) -> Result<String, String> {
        let h = self.session.open(path, flags, FileAttributes::default()).await.map_err(es)?;
        Ok(h.handle)
    }

    pub async fn close(&self, handle: &str) -> Result<(), String> {
        self.session.close(handle).await.map(|_| ()).map_err(es)
    }

    /// 파일 끝이면 `Ok(None)`. 서버는 요청보다 짧게 줄 수 있다(short read) — 호출자가 이어 요청한다.
    pub async fn read_at(&self, handle: &str, offset: u64, len: usize) -> Result<Option<Vec<u8>>, String> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        match self.session.read(handle, offset, len as u32).await {
            Ok(d) => Ok(Some(d.data)),
            Err(SftpError::Status(s)) if s.status_code == StatusCode::Eof => Ok(None),
            Err(e) => Err(es(e)),
        }
    }

    pub async fn write_at(&self, handle: &str, offset: u64, data: Vec<u8>) -> Result<(), String> {
        self.session.write(handle, offset, data).await.map(|_| ()).map_err(es)
    }

    /// 열린 핸들을 주어진 크기로 자른다(이어올리기 전에 구멍 난 꼬리를 버릴 때).
    pub async fn truncate(&self, handle: &str, size: u64) -> Result<(), String> {
        let attrs = FileAttributes { size: Some(size), ..Default::default() };
        self.session.fsetstat(handle, attrs).await.map(|_| ()).map_err(es)
    }

    /// 서버 디스크에 강제 반영(fsync@openssh.com). 미지원이면 조용히 넘어간다.
    pub async fn fsync(&self, handle: &str) {
        if self.feat.fsync {
            let _ = self.session.fsync(handle).await;
        }
    }

    pub async fn stat(&self, path: &str) -> Result<FileAttributes, String> {
        self.session.stat(path).await.map(|a| a.attrs).map_err(es)
    }

    pub async fn setstat(&self, path: &str, attrs: FileAttributes) -> Result<(), String> {
        self.session.setstat(path, attrs).await.map(|_| ()).map_err(es)
    }

    /// 디렉터리 항목 전부(이름, 속성). SFTP는 여러 번 readdir 해야 다 나온다.
    pub async fn list(&self, path: &str) -> Result<Vec<(String, FileAttributes)>, String> {
        let handle = self.session.opendir(path).await.map_err(es)?.handle;
        let mut out = Vec::new();
        let result = loop {
            match self.session.readdir(handle.clone()).await {
                Ok(name) => out.extend(name.files.into_iter().map(|f| (f.filename, f.attrs))),
                Err(SftpError::Status(s)) if s.status_code == StatusCode::Eof => break Ok(out),
                Err(e) => break Err(es(e)),
            }
        };
        let _ = self.session.close(handle).await;
        result
    }

    pub async fn mkdir(&self, path: &str) -> Result<(), String> {
        self.session.mkdir(path, FileAttributes::default()).await.map(|_| ()).map_err(es)
    }

    /// 파일이면 삭제, 아니면 빈 디렉터리 삭제를 시도한다.
    pub async fn remove(&self, path: &str) -> Result<(), String> {
        match self.session.remove(path).await {
            Ok(_) => Ok(()),
            Err(_) => self.session.rmdir(path).await.map(|_| ()).map_err(es),
        }
    }

    /// 이름 바꾸기. `posix-rename`을 지원하면 **원자적 교체**를 쓴다.
    ///
    /// 지원하지 않으면 지우고 옮기는 수밖에 없는데, 그 사이 연결이 끊기면 원본이 사라진다.
    /// 그래서 폴백은 대상이 있을 때만 제거한다.
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        if self.feat.posix_rename {
            let data = ext_two_strings(from, to);
            return match self.session.extended(POSIX_RENAME, data).await.map_err(es)? {
                russh_sftp::protocol::Packet::Status(s) if s.status_code == StatusCode::Ok => Ok(()),
                russh_sftp::protocol::Packet::Status(s) => Err(s.error_message),
                _ => Err("posix-rename: 예기치 않은 응답".into()),
            };
        }
        if self.session.rename(from, to).await.is_ok() {
            return Ok(());
        }
        // SFTP v3 rename은 대상이 있으면 실패한다 — 그때만 제거 후 재시도한다.
        let _ = self.session.remove(to).await;
        self.session.rename(from, to).await.map(|_| ()).map_err(es)
    }

    /// 경로가 속한 파일시스템의 여유 바이트(statvfs 미지원이면 None).
    pub async fn free_space(&self, path: &str) -> Option<u64> {
        if !self.feat.statvfs {
            return None;
        }
        let st = self.session.statvfs(path).await.ok()?;
        Some(st.blocks_avail.saturating_mul(st.fragment_size))
    }
}

#[cfg(test)]
mod tests {
    use super::{cap, ext_two_strings, MAX_CHUNK, SAFE_CHUNK};

    #[test]
    fn chunk_respects_server_limit() {
        assert_eq!(cap(None), SAFE_CHUNK, "한도를 모르면 보수적으로");
        assert_eq!(cap(Some(32 * 1024)), 32 * 1024, "서버가 작게 주면 그대로 따른다");
        assert_eq!(cap(Some(8 * 1024 * 1024)), MAX_CHUNK, "너무 크면 우리 상한으로");
        assert_eq!(cap(Some(16)), 4096, "말도 안 되게 작으면 최소값");
    }

    #[test]
    fn extension_payload_is_two_length_prefixed_strings() {
        // posix-rename의 페이로드 형식이 틀리면 서버가 조용히 거부한다 — 바이트로 못 박는다.
        assert_eq!(ext_two_strings("ab", "c"), vec![0, 0, 0, 2, b'a', b'b', 0, 0, 0, 1, b'c']);
        assert_eq!(ext_two_strings("", ""), vec![0, 0, 0, 0, 0, 0, 0, 0]);
    }
}
