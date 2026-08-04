//! SftpFs — RemoteFs를 SFTP로 구현.

use crate::session::Handler;
use async_trait::async_trait;
use nabi_fs::{FileEntry, FileKind, RemoteFs};
use russh::client::Handle;
use russh_sftp::client::SftpSession;

/// 전송 청크 크기.
///
/// SFTP는 요청/응답이라 처리량이 `청크 ÷ RTT`에 묶인다. 32KiB에서는 RTT 50ms일 때
/// 약 640KB/s가 상한이었다. OpenSSH 클라이언트도 서버와 협상해 ~255KiB까지 올려 쓰며,
/// 서버는 요청이 한도를 넘으면 짧게 잘라 응답하므로(short read) 큰 값이 안전하다.
const XFER_CHUNK: usize = 256 * 1024;

/// SFTP 백엔드. handle을 함께 보관해 세션을 살려둔다.
pub struct SftpFs {
    sftp: SftpSession,
    _handle: Handle<Handler>,
    /// 점프 호스트 핸들(ProxyJump). 드롭되면 터널이 끊기므로 세션 동안 보관(D2).
    _jump: Option<Handle<Handler>>,
    /// 전송 속도 제한(bytes/sec, 0=무제한).
    limit_bps: u64,
    /// true가 되면 진행 중인 전송을 중단(외부에서 set). swap으로 1회성 소비.
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// limit_bps로 보면 지금까지 bytes를 보내는 데 필요한 최소 시간 대비 더 자야 할 시간.
pub(crate) fn throttle_delay(
    bytes: u64,
    elapsed: std::time::Duration,
    limit_bps: u64,
) -> Option<std::time::Duration> {
    if limit_bps == 0 {
        return None;
    }
    std::time::Duration::from_secs_f64(bytes as f64 / limit_bps as f64).checked_sub(elapsed)
}

impl SftpFs {
    pub(crate) fn new(sftp: SftpSession, handle: Handle<Handler>, jump: Option<Handle<Handler>>) -> Self {
        Self {
            sftp,
            _handle: handle,
            _jump: jump,
            limit_bps: 0,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 전송 속도 제한 설정(bytes/sec, 0=무제한).
    pub fn set_limit(&mut self, bps: u64) {
        self.limit_bps = bps;
    }

    /// 외부에서 공유 취소 플래그를 주입(오케스트레이터가 보관한 Arc를 연결).
    pub fn set_cancel(&mut self, c: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        self.cancel = c;
    }

    /// 전송 취소 플래그(클론). 외부(오케스트레이터)가 store(true)하면 다음 청크에서 중단.
    pub fn cancel_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.cancel.clone()
    }

    /// cancel 플래그가 켜졌으면 리셋하고 true(이번 전송 중단).
    fn canceled(&self) -> bool {
        self.cancel.swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// 원격 파일을 로컬로 스트리밍 다운로드하며 누적 바이트를 progress로 보고한다.
    /// 완료 전까지 `{local}.filepart` 임시 파일에 쓰고, 성공 시 원자적으로 rename한다(WinSCP식).
    /// 중단(취소/오류) 시 부분 파일이 `.filepart`로 남아 다음에 이어받기 가능(resume_from 오프셋).
    pub async fn download(
        &mut self,
        remote: &str,
        local: &str,
        resume_from: u64,
        mut progress: impl FnMut(u64),
    ) -> Result<(), String> {
        use std::io::Write;
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let part = format!("{local}.filepart");
        let mut rf = self.sftp.open(remote).await.map_err(|e| e.to_string())?;
        let mut out = if resume_from > 0 {
            rf.seek(std::io::SeekFrom::Start(resume_from))
                .await
                .map_err(|e| e.to_string())?;
            // 이어받기: 남아 있던 .filepart에 append(호출자가 그 크기를 오프셋으로 넘긴다).
            std::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .map_err(|e| e.to_string())?
        } else {
            std::fs::File::create(&part).map_err(|e| e.to_string())?
        };
        let mut buf = vec![0u8; XFER_CHUNK];
        let mut total = resume_from;
        let start = std::time::Instant::now();
        loop {
            let n = rf.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            total += n as u64;
            progress(total);
            if self.canceled() {
                return Err("전송 취소됨".to_string()); // .filepart 유지 → 이어받기.
            }
            if let Some(d) = throttle_delay(total, start.elapsed(), self.limit_bps) {
                tokio::time::sleep(d).await;
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out); // rename 전에 핸들을 닫는다(Windows는 열린 파일을 rename 못 함).
        let meta = self.sftp.metadata(remote).await.ok();
        // 무결성: 원격 크기와 받은 총 바이트가 일치하는지 확인(잘린 전송 감지). stat 미지원 서버는 건너뜀.
        if let Some(rs) = meta.as_ref().and_then(|m| m.size) {
            if total != rs {
                return Err(format!("다운로드 크기 불일치: {total}/{rs} 바이트"));
            }
        }
        std::fs::rename(&part, local).map_err(|e| e.to_string())?;
        // 수정시각 보존: 원격 mtime을 로컬 파일에 적용(서버가 mtime 제공 시).
        if let Some(mt) = meta.as_ref().and_then(|m| m.mtime) {
            if let Ok(f) = std::fs::File::options().write(true).open(local) {
                let _ = f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(mt as u64));
            }
        }
        Ok(())
    }

    /// 로컬 파일을 원격으로 스트리밍 업로드하며 누적 바이트를 progress로 보고한다.
    /// 완료 전까지 `{remote}.filepart`에 올리고 성공 시 대상으로 교체(원자적 갱신) — 실패/취소 시
    /// 기존 원격 파일은 손상되지 않고 임시 파일만 남는다. SFTP v3 rename은 대상 존재 시 실패할 수
    /// 있어 rename 전에 기존 대상을 제거한다(best-effort). 남은 `.filepart`가 있으면 이어서 올린다.
    pub async fn upload(
        &mut self,
        local: &str,
        remote: &str,
        mut progress: impl FnMut(u64),
    ) -> Result<(), String> {
        use russh_sftp::protocol::OpenFlags;
        use std::io::{Read, Seek};
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};
        let part = format!("{remote}.filepart");
        let mut inp = std::fs::File::open(local).map_err(|e| e.to_string())?;
        let local_len = inp.metadata().map_err(|e| e.to_string())?.len();
        // 남은 .filepart(중단된 업로드)가 로컬보다 작으면 그 지점부터 이어서 올린다.
        let resume_from = self.sftp.metadata(&part).await.ok().map(|m| m.len()).unwrap_or(0);
        let mut wf = if resume_from > 0 && resume_from < local_len {
            let mut f = self
                .sftp
                .open_with_flags(&part, OpenFlags::WRITE | OpenFlags::CREATE)
                .await
                .map_err(|e| e.to_string())?;
            f.seek(std::io::SeekFrom::Start(resume_from)).await.map_err(|e| e.to_string())?;
            inp.seek(std::io::SeekFrom::Start(resume_from)).map_err(|e| e.to_string())?;
            f
        } else {
            self.sftp.create(&part).await.map_err(|e| e.to_string())? // 처음부터(truncate).
        };
        let mut buf = vec![0u8; XFER_CHUNK];
        let mut total = resume_from;
        let start = std::time::Instant::now();
        loop {
            let n = inp.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            wf.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
            total += n as u64;
            progress(total);
            if self.canceled() {
                return Err("전송 취소됨".to_string()); // .filepart만 남고 기존 대상은 보존.
            }
            if let Some(d) = throttle_delay(total, start.elapsed(), self.limit_bps) {
                tokio::time::sleep(d).await;
            }
        }
        wf.shutdown().await.map_err(|e| e.to_string())?;
        // 무결성: 올린 원격 임시 파일 크기가 로컬과 일치하는지 확인. stat 미지원 서버는 건너뜀.
        if let Some(rs) = self.sftp.metadata(&part).await.ok().and_then(|m| m.size) {
            if rs != local_len {
                return Err(format!("업로드 크기 불일치: {rs}/{local_len} 바이트"));
            }
        }
        let _ = self.sftp.remove_file(remote).await; // 기존 대상 제거(있으면). 없으면 무시.
        self.sftp.rename(&part, remote).await.map_err(|e| e.to_string())?;
        // 업로드 mtime 보존은 하지 않는다: 일부 서버(Windows OpenSSH sftp-server)가 SETSTAT(mtime)에서
        // 파일을 0바이트로 truncate하는 버그가 있어 데이터 손실 위험. 다운로드 mtime 보존만 제공한다.
        Ok(())
    }
}

#[async_trait]
impl RemoteFs for SftpFs {
    async fn list_dir(&mut self, path: &str) -> Result<Vec<FileEntry>, String> {
        let read_dir = self.sftp.read_dir(path).await.map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for entry in read_dir {
            let ft = entry.file_type();
            let kind = if ft.is_dir() {
                FileKind::Dir
            } else if ft.is_symlink() {
                FileKind::Symlink
            } else if ft.is_file() {
                FileKind::File
            } else {
                FileKind::Other
            };
            let md = entry.metadata();
            out.push(FileEntry {
                name: entry.file_name(),
                kind,
                size: md.len(),
                mode: md.permissions.unwrap_or(0),
                mtime: md.mtime.unwrap_or(0) as u64,
            });
        }
        Ok(out)
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        self.sftp.read(path).await.map_err(|e| e.to_string())
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
        // russh-sftp `write()`는 OpenFlags::WRITE만 써서(CREATE 없음) 실 서버에서 새 파일 생성이
        // 실패한다. create(CREATE|TRUNCATE|WRITE)로 열어 새 파일도 생성되게 한다.
        use tokio::io::AsyncWriteExt;
        let mut f = self.sftp.create(path).await.map_err(|e| e.to_string())?;
        f.write_all(data).await.map_err(|e| e.to_string())?;
        f.shutdown().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn remove(&mut self, path: &str) -> Result<(), String> {
        match self.sftp.remove_file(path).await {
            Ok(()) => Ok(()),
            Err(_) => self.sftp.remove_dir(path).await.map_err(|e| e.to_string()),
        }
    }

    async fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.sftp.rename(from, to).await.map_err(|e| e.to_string())
    }

    async fn mkdir(&mut self, path: &str) -> Result<(), String> {
        self.sftp.create_dir(path).await.map_err(|e| e.to_string())
    }

    async fn chmod(&mut self, path: &str, mode: u32) -> Result<(), String> {
        let attrs = russh_sftp::protocol::FileAttributes {
            permissions: Some(mode),
            ..Default::default()
        };
        self.sftp
            .set_metadata(path, attrs)
            .await
            .map_err(|e| e.to_string())
    }
}

