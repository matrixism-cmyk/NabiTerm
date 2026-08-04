//! SFTP 전송 재시도·자동 재접속(S2 #14/#15) — 연결 오류 시 지수 백오프 후 재접속하고
//! `.filepart` 이어받기로 재시도한다. 취소·비연결 오류는 즉시 실패(무의미한 재시도 방지).

use crate::connsftp::Conn;
use crossbeam_channel::Sender;
use nabi_proto::{Event, SftpId, SshParams};
use nabi_sftp::connect_sftp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 최대 재시도 횟수(초기 시도 제외).
const MAX_RETRIES: u32 = 3;
/// 진행 이벤트 합치기 임계(바이트) — 액터 기존값과 동일.
const PROGRESS_STEP: u64 = 262_144;

/// 지수 백오프(ms): 500·2^attempt, 최대 8초. 순수 함수.
pub(crate) fn backoff_ms(attempt: u32) -> u64 {
    (500u64.saturating_mul(1u64 << attempt.min(4))).min(8000)
}

/// 오류 문자열이 (재시도 가치가 있는) 연결/일시 오류로 보이는지. 취소·권한·경로 오류는 false.
pub(crate) fn is_connection_error(err: &str) -> bool {
    if err.contains("취소") {
        return false; // 사용자 취소는 재시도 금지.
    }
    let e = err.to_lowercase();
    [
        "connection", "reset", "broken pipe", "eof", "closed", "timeout",
        "disconnect", "unexpected", "연결", "시간 초과",
    ]
    .iter()
    .any(|k| e.contains(k))
}

/// 남은 `{local}.filepart` 크기(다음 재시도의 이어받기 오프셋).
fn filepart_len(local: &str) -> u64 {
    std::fs::metadata(format!("{local}.filepart")).map(|m| m.len()).unwrap_or(0)
}

/// 새 SFTP 연결을 만들어 속도 제한·취소 플래그를 연결한다(재접속용). 실패 시 None.
async fn reconnect_sftp(params: &SshParams, limit_kbps: u32, cancel: Arc<AtomicBool>) -> Option<Conn> {
    connect_sftp(params).await.ok().map(|mut f| {
        f.set_limit(limit_kbps as u64 * 1024);
        f.set_cancel(cancel);
        Conn::Sftp(f)
    })
}

/// 재시도 계속 여부: 취소 안 됨 + 시도 남음 + 연결 오류 + SFTP 연결일 때만.
fn should_retry(fs: &Conn, cancel: &Arc<AtomicBool>, attempt: u32, err: &str) -> bool {
    !cancel.load(Ordering::Relaxed) && attempt < MAX_RETRIES && fs.is_sftp() && is_connection_error(err)
}

/// 다운로드를 실패 시 재접속+이어받기로 재시도하며 실행한다.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_download(
    fs: &mut Conn,
    params: &SshParams,
    limit_kbps: u32,
    cancel: &Arc<AtomicBool>,
    id: SftpId,
    remote: &str,
    local: &str,
    mut resume: u64,
    ev: &Sender<Event>,
) -> Result<(), String> {
    let mut attempt = 0u32;
    loop {
        let mut last = resume;
        let res = fs
            .download(remote, local, resume, |total| {
                if total - last >= PROGRESS_STEP {
                    last = total;
                    let _ = ev.send(Event::SftpProgress { id, bytes: total });
                }
            })
            .await;
        match res {
            Ok(()) => return Ok(()),
            Err(e) if should_retry(fs, cancel, attempt, &e) => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(backoff_ms(attempt))).await;
                if let Some(newfs) = reconnect_sftp(params, limit_kbps, cancel.clone()).await {
                    *fs = newfs;
                }
                resume = filepart_len(local); // 남은 부분부터 이어받기.
            }
            Err(e) => return Err(e),
        }
    }
}

/// 업로드를 실패 시 재접속으로 재시도하며 실행한다(백엔드가 원격 .filepart에서 자동 재개).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_upload(
    fs: &mut Conn,
    params: &SshParams,
    limit_kbps: u32,
    cancel: &Arc<AtomicBool>,
    id: SftpId,
    local: &str,
    remote: &str,
    ev: &Sender<Event>,
) -> Result<(), String> {
    let mut attempt = 0u32;
    loop {
        let mut last = 0u64;
        let res = fs
            .upload(local, remote, |total| {
                if total - last >= PROGRESS_STEP {
                    last = total;
                    let _ = ev.send(Event::SftpProgress { id, bytes: total });
                }
            })
            .await;
        match res {
            Ok(()) => return Ok(()),
            Err(e) if should_retry(fs, cancel, attempt, &e) => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(backoff_ms(attempt))).await;
                if let Some(newfs) = reconnect_sftp(params, limit_kbps, cancel.clone()).await {
                    *fs = newfs;
                }
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{backoff_ms, is_connection_error};

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_ms(1), 1000); // 500·2
        assert_eq!(backoff_ms(2), 2000);
        assert_eq!(backoff_ms(3), 4000);
        assert_eq!(backoff_ms(4), 8000);
        assert_eq!(backoff_ms(10), 8000); // 상한.
    }

    #[test]
    fn classifies_errors() {
        assert!(is_connection_error("Connection reset by peer"));
        assert!(is_connection_error("연결 시간 초과"));
        assert!(is_connection_error("unexpected EOF"));
        assert!(!is_connection_error("전송 취소됨")); // 취소는 재시도 안 함.
        assert!(!is_connection_error("Permission denied"));
        assert!(!is_connection_error("No such file"));
    }
}
