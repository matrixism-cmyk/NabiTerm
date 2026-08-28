//! 파일 전송 — 원자적 교체(.filepart)·이어받기·무결성 검사. 파이프라이닝은 pipeline.rs.
//!
//! 두 방향 모두 임시 이름으로 받아/올린 뒤 마지막에 대상으로 옮긴다. 그래서 전송이 끊겨도
//! 기존 파일이 반쯤 덮인 상태로 남지 않는다(WinSCP·rclone과 같은 방식).

use crate::fs::{throttle_delay, SftpFs};
use crate::pipeline::{depth, download_stream, resume_offset, upload_stream};
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use std::io::{Read, Seek, Write};

impl SftpFs {
    /// 파도마다 부를 콜백 — 취소를 확인하고 속도 제한만큼 잘 시간을 돌려준다.
    fn ticker(
        &self,
        start: std::time::Instant,
    ) -> impl FnMut(u64) -> Result<Option<std::time::Duration>, String> + '_ {
        move |done: u64| {
            if self.canceled() {
                return Err(nabi_i18n::trc("net.xfer.canceled").to_string());
            }
            Ok(throttle_delay(done, start.elapsed(), self.limit_bps))
        }
    }

    /// 원격 파일을 로컬로 내려받으며 누적 바이트를 progress로 보고한다.
    ///
    /// 완료 전까지 `{local}.filepart`에 쓰고 성공 시 원자적으로 옮긴다. 중단되면 부분 파일이
    /// 남아 다음에 이어받을 수 있다(호출자가 그 크기를 `resume_from`으로 넘긴다).
    pub async fn download(
        &mut self,
        remote: &str,
        local: &str,
        resume_from: u64,
        mut progress: impl FnMut(u64),
    ) -> Result<(), String> {
        let part = format!("{local}.filepart");
        // 크기를 미리 알면 파일 끝 너머로 헛요청하지 않고, 끝에서 EOF 확인 왕복도 아낀다.
        let attrs = self.raw.stat(remote).await.ok();
        let handle = self.raw.open(remote, OpenFlags::READ).await?;
        let mut out = if resume_from > 0 {
            std::fs::OpenOptions::new().append(true).open(&part).map_err(|e| e.to_string())?
        } else {
            std::fs::File::create(&part).map_err(|e| e.to_string())?
        };
        let raw = self.raw.clone();
        let start = std::time::Instant::now();
        let mut tick = self.ticker(start);
        let got = download_stream(
            &raw,
            &handle,
            resume_from,
            attrs.as_ref().and_then(|a| a.size),
            |d| out.write_all(d).map_err(|e| e.to_string()),
            // 파도마다 진행률을 보고한다. 예전에는 끝나고 한 번만 불러서 큐의 진행 막대가
            // 0%에 멈춰 있다가 갑자기 100%가 됐다 — 큰 파일일수록 멈춘 것처럼 보였다.
            |done| {
                progress(resume_from + done);
                tick(resume_from + done)
            },
        )
        .await;
        let _ = raw.close(&handle).await;
        let got = got?;
        out.flush().map_err(|e| e.to_string())?;
        drop(out); // 옮기기 전에 핸들을 닫는다(Windows는 열린 파일을 옮기지 못한다).
        progress(resume_from + got);

        let attrs = raw.stat(remote).await.ok();
        // 무결성: 원격 크기와 받은 총 바이트가 같아야 한다. stat 미지원 서버는 건너뛴다.
        if let Some(rs) = attrs.as_ref().and_then(|a| a.size) {
            if resume_from + got != rs {
                return Err(nabi_i18n::trc("net.xfer.dlsize").replace("{a}", &(resume_from + got).to_string()).replace("{b}", &rs.to_string()));
            }
        }
        // 무결성(옵션): 원격 sha256sum과 받은 파일의 SHA-256 대조 — 실패 시 대상 파일을
        // 건드리지 않고 임시(.filepart)만 남긴다. 해시 명령이 없는 서버는 건너뛰되, 건너뛴 사실을 세어 화면이 알린다(배치 AF).
        if crate::hashcheck::enabled() {
            if let Some(rh) = crate::hashcheck::remote_sha256(&self.handle, remote, &self.hash_probe).await {
                let lh = crate::hashcheck::local_sha256(&part)?;
                if lh != rh {
                    return Err(nabi_i18n::trc("net.xfer.dlhash").replace("{l}", &lh).replace("{r}", &rh));
                }
            }
        }
        std::fs::rename(&part, local).map_err(|e| e.to_string())?;
        // 수정시각 보존: 원격 mtime을 로컬 파일에 적용(서버가 mtime을 줄 때만).
        if let Some(mt) = attrs.as_ref().and_then(|a| a.mtime) {
            if let Ok(f) = std::fs::File::options().write(true).open(local) {
                let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(mt as u64);
                let _ = f.set_modified(when);
            }
        }
        Ok(())
    }

    /// 로컬 파일을 원격으로 올리며 누적 바이트를 progress로 보고한다.
    ///
    /// `{remote}.filepart`에 올린 뒤 대상으로 교체한다. 실패/취소 시 기존 원격 파일은
    /// 그대로 남고 임시 파일만 남는다. 남은 임시 파일이 있으면 이어서 올린다.
    pub async fn upload(
        &mut self,
        local: &str,
        remote: &str,
        mut progress: impl FnMut(u64),
    ) -> Result<(), String> {
        let part = format!("{remote}.filepart");
        let mut inp = std::fs::File::open(local).map_err(|e| e.to_string())?;
        let local_len = inp.metadata().map_err(|e| e.to_string())?.len();
        self.check_space(remote, local_len).await?;

        // 남은 임시 파일이 로컬보다 작으면 이어서 올린다. 다만 마지막 파도는 구멍이 있을 수
        // 있으므로 그만큼 뒤로 물러난 지점부터 다시 보낸다(pipeline::resume_offset).
        let part_len = self.raw.stat(&part).await.ok().map(|a| a.len()).unwrap_or(0);
        let resume = if part_len > 0 && part_len < local_len {
            resume_offset(part_len, self.raw.write_chunk())
        } else {
            0
        };
        let flags = if resume > 0 {
            OpenFlags::WRITE | OpenFlags::CREATE
        } else {
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE
        };
        let handle = self.raw.open(&part, flags).await?;
        if resume > 0 {
            // 꼬리를 자르지 않는다. `resume`부터 파일 끝까지 **전부 다시 쓰므로** 구멍이 남을
            // 수 없고, Windows OpenSSH sftp-server는 열린 핸들의 fsetstat(size)를 거부한다
            // (실서버 확인 2026-08-05: "Failure"). 자르기에 기대면 그 서버에서 이어올리기가
            // 통째로 실패한다.
            inp.seek(std::io::SeekFrom::Start(resume)).map_err(|e| e.to_string())?;
        }

        let raw = self.raw.clone();
        let chunk = raw.write_chunk();
        let mut buf = vec![0u8; chunk];
        let start = std::time::Instant::now();
        let mut tick = self.ticker(start);
        // 다운로드와 같은 이유로 파도마다 보고한다(끝나고 한 번만 부르면 막대가 안 움직인다).
        let mut report = |done: u64| {
            progress(done);
            tick(done)
        };
        let sent = upload_stream(
            &raw,
            &handle,
            resume,
            |want| {
                let n = read_full(&mut inp, &mut buf[..want.min(chunk)])?;
                Ok(buf[..n].to_vec())
            },
            &mut report,
        )
        .await;
        let acked = match &sent {
            Ok(n) => *n,
            Err((n, _)) => *n,
        };
        if sent.is_err() {
            // 확정된 접두 길이로 잘라 두면 다음 이어올리기가 깔끔하다. 지원하지 않는 서버가
            // 있어 실패는 무시한다 — 정확성은 resume_offset의 되감기가 이미 보장한다.
            let _ = raw.truncate(&handle, acked).await;
        }
        raw.fsync(&handle).await; // 이름을 바꾸기 전에 서버 디스크로 밀어낸다.
        let _ = raw.close(&handle).await;
        if let Err((_, e)) = sent {
            return Err(e);
        }
        progress(acked);

        // 무결성: 올린 임시 파일 크기가 로컬과 같아야 한다. stat 미지원 서버는 건너뛴다.
        if let Some(rs) = raw.stat(&part).await.ok().and_then(|a| a.size) {
            if rs != local_len {
                return Err(nabi_i18n::trc("net.xfer.upsize").replace("{a}", &rs.to_string()).replace("{b}", &local_len.to_string()));
            }
        }
        // 무결성(옵션): 올린 임시 파일의 원격 해시와 로컬 원본 대조 — 교체 전에 잡는다.
        if crate::hashcheck::enabled() {
            if let Some(rh) = crate::hashcheck::remote_sha256(&self.handle, &part, &self.hash_probe).await {
                let lh = crate::hashcheck::local_sha256(local)?;
                if lh != rh {
                    return Err(nabi_i18n::trc("net.xfer.uphash").replace("{l}", &lh).replace("{r}", &rh));
                }
            }
        }
        raw.rename(&part, remote).await?;
        // 권한 정규화(옵션, 기본 꺼짐): 올린 스크립트에 실행 비트를 준다.
        // 실패는 무시한다 — 전송은 이미 끝났고, SETSTAT을 막는 서버도 있다.
        if let Some(mode) = crate::uploadmode::mode_for(&crate::upload_mode(), remote) {
            let _ = raw.setstat(remote, FileAttributes { permissions: Some(mode), ..Default::default() }).await;
        }
        Ok(())
        // 업로드 mtime 보존은 하지 않는다: 일부 서버(Windows OpenSSH sftp-server)가 SETSTAT(mtime)에서
        // 파일을 0바이트로 truncate하는 버그가 있어 데이터 손실 위험. 다운로드 mtime 보존만 제공한다.
        // 권한만 담은 SETSTAT은 chmod 기능에서 이미 쓰고 있어(같은 요청) 그 위험이 없다.
    }

    /// 올리기 전에 여유 공간을 확인한다(statvfs 지원 서버만). 전송 도중 ENOSPC로 죽는 대신
    /// 시작 전에 알려 준다. 한 파도 분량은 여유로 남겨 둔다.
    async fn check_space(&self, remote: &str, need: u64) -> Result<(), String> {
        let dir = remote.rsplit_once('/').map(|(d, _)| d).filter(|d| !d.is_empty()).unwrap_or("/");
        let Some(free) = self.raw.free_space(dir).await else {
            return Ok(());
        };
        let margin = (depth() * self.raw.write_chunk()) as u64;
        if free < need.saturating_add(margin) {
            return Err(nabi_i18n::trc("net.xfer.nospace").replace("{free}", &free.to_string()).replace("{need}", &need.to_string()));
        }
        Ok(())
    }
}

/// 버퍼가 찰 때까지(또는 EOF까지) 읽는다. `Read::read`는 짧게 돌아올 수 있어 그대로 쓰면
/// 청크가 들쭉날쭉해지고 파이프라인 오프셋 계산이 복잡해진다.
fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> Result<usize, String> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]).map_err(|e| e.to_string())? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}
