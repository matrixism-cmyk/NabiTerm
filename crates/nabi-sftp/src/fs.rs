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
    ///
    /// `Arc` 인 이유: 터미널 SSH 연결을 **그대로 받아 쓰기** 위해서다(배치 Y H5).
    /// pane 을 닫아도 여기서 부드는 동안은 연결이 살아 있어 진행 중인 전송이 끊기지 않는다.
    pub(crate) handle: std::sync::Arc<Handle<Handler>>,
    /// 점프 호스트 핸들(ProxyJump). 드롭되면 터널이 끊기므로 세션 동안 보관(D2).
    _jump: Vec<std::sync::Arc<Handle<Handler>>>,
    /// 전송 속도 제한(bytes/sec, 0=무제한).
    pub(crate) limit_bps: u64,
    /// true가 되면 진행 중인 전송을 중단(외부에서 set). swap으로 1회성 소비.
    pub(crate) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 이 연결에서 원격 해시 명령을 쓸 수 있는지 — **한 번만 물어본다**(배치 AF).
    pub(crate) hash_probe: crate::hashcheck::HashProbe,
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
    /// 이 세션이 타고 있는 SSH 연결 — **다른 SFTP가 물려받을 수 있게** 내준다(배치 Y H5).
    ///
    /// `Arc` 사본이라 이 세션을 닫아도 받은 쪽은 계속 쓴다. 반대도 마찬가지다.
    pub fn handle_for_reuse(&self) -> std::sync::Arc<Handle<Handler>> {
        self.handle.clone()
    }

    /// 점프 호스트 핸들 — 목적지 핸들과 **함께** 물려준다.
    ///
    /// **왜 "반드시"라고 적지 않는가.** 실서버로 확인해 보니 이것을 일부러 빠뜨려도 터널이
    /// 곧바로 끊기지는 않았다(SSH 라이브러리의 배경 태스크가 세션을 붙들고 있다). 그러니
    /// "빠뜨리면 죽는다"고 단정하지 않는다 — 그렇게 적었다가 시험으로 반증했다.
    ///
    /// 그래도 함께 넘기는 이유: 그 수명이 우리가 정하지 않은 구현 세부에 기대고 있고, 그
    /// 세부는 예고 없이 바뀔 수 있다. 잡고 있는 값은 싸다.
    pub fn jump_for_reuse(&self) -> Vec<std::sync::Arc<Handle<Handler>>> {
        self._jump.clone()
    }

    pub(crate) fn new(
        raw: RawFs,
        handle: std::sync::Arc<Handle<Handler>>,
        jump: Vec<std::sync::Arc<Handle<Handler>>>,
    ) -> Self {
        Self {
            hash_probe: Default::default(),
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

    /// 서버 안에서 파일을 복사한다 — **디스크를 거치지 않는다.**
    ///
    /// 예전에는 받았다가 다시 올려야 했다. 그러면 회선을 두 번 타는 것에 더해 큰 파일에서는
    /// 임시 파일이 디스크 한 벌을 차지하고, 중간에 끊기면 그 찌꺼기가 남는다.
    ///
    /// 여기서는 서버에서 조각을 읽어 곧바로 서버로 되쓴다. 회선은 여전히 두 번 타지만
    /// (SFTP v3에는 서버 안에서 옮기라고 시킬 방법이 없다) **디스크는 건드리지 않는다.**
    /// OpenSSH 9 이상의 `copy-data` 확장을 쓰면 회선도 안 타는데, 그건 서버가 알려 줄
    /// 때만 되므로 후속으로 미룬다 — 지금 것은 **어느 서버에서나** 된다.
    async fn copy_remote(
        &mut self,
        from: &str,
        to: &str,
        tick: &mut (dyn FnMut(u64) + Send),
    ) -> Result<u64, String> {
        let src = self.raw.open(from, OpenFlags::READ).await?;
        let dst = match self
            .raw
            .open(to, OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE)
            .await
        {
            Ok(h) => h,
            // 대상을 못 열면 원본 손잡이를 두고 나가지 않는다 — 서버의 열린 파일 수는 한정돼 있다.
            Err(e) => {
                let _ = self.raw.close(&src).await;
                return Err(e);
            }
        };
        let chunk = self.raw.read_chunk();
        let mut pos = 0u64;
        let mut err = None;
        loop {
            // 취소는 전송과 같은 깃발을 쓴다(사용자가 멈추라면 여기서도 멈춘다).
            if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                err = Some("취소됨".to_string());
                break;
            }
            match self.raw.read_at(&src, pos, chunk).await {
                Ok(None) => break,
                Ok(Some(d)) if d.is_empty() => break,
                Ok(Some(d)) => {
                    let n = d.len() as u64;
                    if let Err(e) = self.raw.write_at(&dst, pos, d).await {
                        err = Some(e);
                        break;
                    }
                    pos += n;
                    tick(pos);
                }
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        self.raw.fsync(&dst).await;
        let _ = self.raw.close(&dst).await;
        let _ = self.raw.close(&src).await;
        match err {
            // 반쯤 쓰다 만 파일을 남기지 않는다 — 있으면 성공한 복사로 오해한다.
            Some(e) => {
                let _ = self.raw.remove(to).await;
                Err(e)
            }
            None => Ok(pos),
        }
    }

    /// 같은 SSH 연결에 exec 채널을 하나 열어 명령을 돌린다.
    ///
    /// SFTP 연결이 이미 SSH 위에 있으므로 **새로 붙지 않는다** — 다시 인증하지 않고,
    /// 점프 호스트를 다시 타지도 않는다.
    ///
    /// 표준오류를 함께 모으는 이유: 명령이 실패하면 할 말은 대개 그쪽에 있다. 따로 두면
    /// 화면이 비어 보이고 사용자는 아무 일도 안 일어난 줄 안다.
    ///
    /// `max`를 넘으면 **거기서 그만 모은다.** 서버가 기가바이트를 뱉는 명령을 돌릴 수도
    /// 있는데, 그걸 다 담으면 우리가 죽는다.
    async fn exec_remote(&mut self, cmd: &str, max: usize) -> Result<(String, Option<i32>), String> {
        use russh::ChannelMsg;
        let mut ch = self.handle.channel_open_session().await.map_err(|e| e.to_string())?;
        ch.exec(true, cmd).await.map_err(|e| e.to_string())?;
        let (mut buf, mut code, mut cut) = (Vec::new(), None, false);
        loop {
            match ch.wait().await {
                // 표준출력과 표준오류를 한 흐름으로 모은다(서버가 보낸 순서 그대로).
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    if buf.len() < max {
                        let room = max - buf.len();
                        let take = room.min(data.len());
                        buf.extend_from_slice(&data[..take]);
                        cut |= take < data.len();
                    } else {
                        cut = true;
                    }
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => code = Some(exit_status as i32),
                // **Eof에서 멈추면 안 된다.** 서버는 대개 Eof를 먼저 보내고 그 뒤에
                // ExitStatus를 보낸다 — 여기서 끊으면 출력은 오는데 **종료 코드만 늘 빈다.**
                // 실서버 시험이 이것을 잡았다(인프로세스 서버는 exec 자체가 없다).
                Some(ChannelMsg::Eof) => {}
                Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
        let mut out = String::from_utf8_lossy(&buf).into_owned();
        if cut {
            // 조용히 자르면 "이게 전부"로 읽힌다. 잘렸다는 사실은 화면에서 번역한다.
            out.push_str("\n[exec.truncated]\n");
        }
        Ok((out, code))
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
