//! SftpFs — RemoteFs를 SFTP로 구현. 파일 전송(다운로드/업로드)은 xfer.rs.

use crate::raw::RawFs;
use crate::session::Handler;
use async_trait::async_trait;
use nabi_fs::{FileEntry, FileKind, RemoteFs};
use russh::client::Handle;
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags};

/// SFTP 백엔드. handle을 함께 보관해 세션을 살려둔다.
pub struct SftpFs {
    pub(crate) raw: RawFs,
    /// SSH 핸들 — 세션 유지 + 원격 해시 명령(hashcheck) 실행에 쓴다.
    pub(crate) handle: Handle<Handler>,
    /// 점프 호스트 핸들(ProxyJump). 드롭되면 터널이 끊기므로 세션 동안 보관(D2).
    _jump: Option<Handle<Handler>>,
    /// 전송 속도 제한(bytes/sec, 0=무제한).
    pub(crate) limit_bps: u64,
    /// true가 되면 진행 중인 전송을 중단(외부에서 set). swap으로 1회성 소비.
    pub(crate) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
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

/// SFTP 속성 → 우리 파일 종류.
fn kind_of(a: &FileAttributes) -> FileKind {
    match a.file_type() {
        FileType::Dir => FileKind::Dir,
        FileType::Symlink => FileKind::Symlink,
        FileType::File => FileKind::File,
        FileType::Other => FileKind::Other,
    }
}

impl SftpFs {
    pub(crate) fn new(raw: RawFs, handle: Handle<Handler>, jump: Option<Handle<Handler>>) -> Self {
        Self {
            raw,
            handle,
            _jump: jump,
            limit_bps: 0,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 링크가 가리키는 것이 폴더인지 알아내 `Dir`로 바꾼다(표시는 그대로 링크).
    ///
    /// `readdir`는 링크 자신의 속성을 주므로, 따라가는 `stat`을 한 번 더 물어야 한다.
    /// 확인하지 못한 링크는 **건드리지 않는다** — 모르는 것을 폴더로 단정하지 않는다.
    async fn resolve_links(&self, dir: &str, out: &mut [FileEntry]) {
        let kinds: Vec<FileKind> = out.iter().map(|e| e.kind).collect();
        let names: Vec<String> = out.iter().map(|e| e.name.clone()).collect();
        let idx = crate::linkres::to_resolve(&kinds, &names);
        // 상한에 걸린 만큼은 링크 그대로 남는다. 조용히 넘기면 나중에 "왜 이 링크만
        // 안 들어가지느냐"를 아무도 설명하지 못하므로 로그에는 남긴다.
        let left = crate::linkres::unresolved(&kinds);
        if left > 0 {
            tracing::info!(dir = %dir, unresolved = left, "링크 대상 확인 상한 초과");
        }
        for chunk in idx.chunks(crate::linkres::BATCH) {
            // 지연이 큰 회선에서 하나씩 물으면 개수 × 왕복이 그대로 대기가 된다.
            let reqs = chunk.iter().map(|i| {
                let p = crate::linkres::target_path(dir, &out[*i].name);
                let raw = self.raw.clone();
                async move { raw.stat(&p).await }
            });
            for (n, r) in futures_util::future::join_all(reqs).await.into_iter().enumerate() {
                // 실패는 조용히 넘긴다 — 끊어진 링크(dangling)는 흔하고, 그것 때문에
                // 목록 전체를 실패시키면 폴더를 아예 못 연다.
                if let Ok(a) = r {
                    if matches!(a.file_type(), FileType::Dir) {
                        out[chunk[n]].kind = FileKind::LinkDir;
                    }
                }
            }
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

    /// 전송 취소 플래그(클론). 외부(오케스트레이터)가 store(true)하면 다음 파도에서 중단.
    pub fn cancel_flag(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.cancel.clone()
    }

    /// cancel 플래그가 켜졌으면 리셋하고 true(이번 전송 중단).
    pub(crate) fn canceled(&self) -> bool {
        self.cancel.swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// 원격 파일시스템의 여유 공간(statvfs 미지원 서버는 None).
    pub async fn free_space(&self, path: &str) -> Option<u64> {
        self.raw.free_space(path).await
    }
    /// 파일 **앞부분만** 읽는다(미리보기). `(바이트, 더 있는가)`.
    ///
    /// 크기를 묻지 않고 상한만큼만 읽는 것이 요점이다. 원격 파일 크기는 믿을 수 없다 —
    /// 심볼릭 링크, /proc 같은 가짜 파일, 잘못된 stat이 흔하다. "크기를 보고 작으면 다
    /// 읽자"는 길을 아예 만들지 않으면 몇 GB를 실수로 끌어올 일이 없다.
    ///
    /// 한 번의 read로 상한을 다 못 채울 수 있어(서버가 청크를 쪼갠다) 채울 때까지 반복하되,
    /// **상한을 넘겨 읽지는 않는다.**
    pub async fn preview(&self, path: &str, max: usize) -> Result<(Vec<u8>, bool), String> {
        let h = self.raw.open(path, OpenFlags::READ).await?;
        let mut out: Vec<u8> = Vec::with_capacity(max.min(64 * 1024));
        let mut more = false;
        while out.len() < max {
            let want = max - out.len();
            match self.raw.read_at(&h, out.len() as u64, want).await {
                Ok(Some(chunk)) if !chunk.is_empty() => out.extend_from_slice(&chunk),
                Ok(_) => break,        // 파일 끝.
                Err(e) => {
                    let _ = self.raw.close(&h).await;
                    return Err(e);
                }
            }
        }
        // 상한을 채웠다면 뒤에 더 있는지 한 바이트로 확인한다(있다고 넘겨짚지 않는다 —
        // 딱 max 바이트짜리 파일에 "더 있음"을 띄우면 거짓말이다).
        if out.len() >= max {
            more = matches!(self.raw.read_at(&h, max as u64, 1).await, Ok(Some(b)) if !b.is_empty());
        }
        let _ = self.raw.close(&h).await;
        Ok((out, more))
    }
}

#[async_trait]
impl RemoteFs for SftpFs {
    async fn list_dir(&mut self, path: &str) -> Result<Vec<FileEntry>, String> {
        let files = self.raw.list(path).await?;
        let mut out: Vec<FileEntry> = files
            .into_iter()
            .map(|(name, a)| FileEntry {
                name,
                kind: kind_of(&a),
                size: a.len(),
                mode: a.permissions.unwrap_or(0),
                mtime: a.mtime.unwrap_or(0) as u64,
            })
            .collect();
        self.resolve_links(path, &mut out).await;
        Ok(out)
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        let size = self.raw.stat(path).await.ok().and_then(|a| a.size);
        let h = self.raw.open(path, OpenFlags::READ).await?;
        let mut out = Vec::new();
        let raw = self.raw.clone();
        let r = crate::pipeline::download_stream(
            &raw,
            &h,
            0,
            size,
            |d| {
                out.extend_from_slice(d);
                Ok(())
            },
            |_| Ok(None),
        )
        .await;
        let _ = self.raw.close(&h).await;
        r.map(|_| out)
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
        let flags = OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE;
        let h = self.raw.open(path, flags).await?;
        let mut off = 0usize;
        let chunk = self.raw.write_chunk();
        let mut result = Ok(());
        while off < data.len() {
            let end = (off + chunk).min(data.len());
            if let Err(e) = self.raw.write_at(&h, off as u64, data[off..end].to_vec()).await {
                result = Err(e);
                break;
            }
            off = end;
        }
        self.raw.fsync(&h).await;
        let _ = self.raw.close(&h).await;
        result
    }

    async fn remove(&mut self, path: &str) -> Result<(), String> {
        self.raw.remove(path).await
    }

    async fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.raw.rename(from, to).await
    }

    async fn mkdir(&mut self, path: &str) -> Result<(), String> {
        self.raw.mkdir(path).await
    }

    async fn chmod(&mut self, path: &str, mode: u32) -> Result<(), String> {
        let attrs = FileAttributes { permissions: Some(mode), ..Default::default() };
        self.raw.setstat(path, attrs).await
    }
}

#[cfg(test)]
mod tests {
    use super::throttle_delay;
    use std::time::Duration;

    #[test]
    fn throttle_only_when_ahead_of_schedule() {
        assert_eq!(throttle_delay(1000, Duration::from_secs(0), 0), None, "무제한이면 안 잔다");
        // 1000B를 1000B/s로 보내려면 1초 — 0.2초밖에 안 지났으면 0.8초 더 자야 한다.
        let d = throttle_delay(1000, Duration::from_millis(200), 1000).expect("지연 필요");
        assert!((d.as_secs_f64() - 0.8).abs() < 0.01);
        assert_eq!(throttle_delay(1000, Duration::from_secs(2), 1000), None, "이미 늦었으면 안 잔다");
    }
}
