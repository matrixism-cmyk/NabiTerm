//! 전송 무결성 해시 검증(T2-1, rclone 방식) — 크기 비교 위의 한 단계.
//!
//! rclone처럼 폴백 사다리를 탄다: ① 원격에서 `sha256sum` 계열 명령을 실행해 해시를 얻고
//! 로컬 해시와 대조 ② 명령이 없는 서버(Windows OpenSSH 등)면 건너뛴다. 크기 비교
//! (xfer.rs)는 항상 수행되므로 최소선은 유지된다.
//!
//! **다만 조용히 건너뛰지는 않는다**(배치 AF). 검증한 파일과 못 한 파일을 따로 세어
//! 화면이 구분해 보여 준다. 구분이 안 되면 검증을 켜 둔 사용자는 **아무것도 검증되지 않은
//! 전송을 검증된 것으로 믿는다** — 신뢰가 전부인 기능에서 그것이 가장 나쁜 실패다.
//!
//! 켜고 끄기는 전역 스위치(설정 ▸ 터미널)로 한다. 워커 풀이 만드는 별도 연결도
//! 같은 스위치를 읽으므로 흐름마다 플래그를 실어 나를 필요가 없다(SSH_KEEPALIVE_SECS 전례).

use crate::session::Handler;
use russh::client::Handle;
use russh::ChannelMsg;
use std::sync::atomic::{AtomicBool, Ordering};

/// 전송 후 SHA-256 해시 검증을 할지(설정에서 갱신, 전송 시 읽음). 기본 끔 —
/// 원격 해시 명령이 파일 전체를 다시 읽으므로 큰 파일에서 시간이 배로 든다.
pub static SFTP_VERIFY_HASH: AtomicBool = AtomicBool::new(false);

/// 이 세션에서 **검증에 성공한** 파일 수.
pub static VERIFIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 이 세션에서 **검증하지 못하고 넘어간** 파일 수(서버에 해시 명령이 없음).
///
/// 이 숫자가 있어야 "검증됨"과 "검증 못 함"을 구분할 수 있다. 구분이 안 되면 검증을 켜 둔
/// 사용자는 **아무것도 검증되지 않은 전송을 검증된 것으로 믿는다** — 신뢰가 전부인 기능에서
/// 그것이 가장 나쁜 실패다. 화면이 매 프레임 물어도 싸도록 세어만 둔다.
pub static SKIPPED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// (검증됨, 검증 못 함) — 화면이 읽는다.
pub fn tally() -> (usize, usize) {
    (
        VERIFIED.load(Ordering::Relaxed),
        SKIPPED.load(Ordering::Relaxed),
    )
}

/// 이 연결에서 원격 해시 명령을 쓸 수 있는가 — **한 번만 물어본다.**
///
/// 예전에는 파일마다 `sha256sum` 과 `shasum` 을 차례로 시도했다. 명령이 없는 서버
/// (윈도우 OpenSSH 등)에서는 **파일 하나마다 exec 채널 두 개를 열고 두 번 다 실패**했다.
/// 500개를 동기화하면 1000번의 헛된 왕복이다. 답은 연결이 사는 동안 바뀌지 않으므로
/// 한 번 알아내면 된다.
#[derive(Debug, Default)]
pub(crate) struct HashProbe(std::sync::atomic::AtomicU8);

impl HashProbe {
    /// 0=아직 모름 · 1=쓸 수 있다 · 2=없다.
    pub(crate) fn known(&self) -> Option<bool> {
        match self.0.load(Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    pub(crate) fn set(&self, has: bool) {
        self.0.store(if has { 1 } else { 2 }, Ordering::Relaxed);
    }
}

pub(crate) fn enabled() -> bool {
    SFTP_VERIFY_HASH.load(Ordering::Relaxed)
}

/// POSIX 셸 단일 인용 — 경로에 어떤 문자가 있어도 명령 주입이 되지 않게.
pub(crate) use nabi_proto::shquote::shell_quote;

/// `sha256sum`/`shasum` 출력에서 64자리 16진 해시만 뽑는다(형식: `<hex>  <path>`).
pub(crate) fn parse_hash_output(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|t| t.to_ascii_lowercase())
}

/// 원격 파일의 SHA-256을 셸 명령으로 얻는다. 명령이 없거나 실패하면 None(폴백 사다리).
///
/// `probe` 는 **이 연결에서 해시 명령을 쓸 수 있는지** 기억한다. 없다고 한 번 밝혀지면
/// 다시 묻지 않는다 — 예전에는 파일마다 두 번씩 헛되이 물었다.
///
/// 세어 두는 이유: 검증한 파일과 **검증하지 못한 파일**을 화면이 구분해 보여 줄 수 있어야
/// 한다. 조용히 건너뛰면 사용자는 검증되지 않은 전송을 검증된 것으로 믿는다.
pub(crate) async fn remote_sha256(
    handle: &Handle<Handler>,
    path: &str,
    probe: &HashProbe,
) -> Option<String> {
    if probe.known() == Some(false) {
        SKIPPED.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    let q = shell_quote(path);
    // GNU coreutils → BSD/macOS 순으로 시도.
    for cmd in [format!("sha256sum -- {q}"), format!("shasum -a 256 -- {q}")] {
        if let Some(h) = exec_hash(handle, &cmd).await {
            probe.set(true);
            VERIFIED.fetch_add(1, Ordering::Relaxed);
            return Some(h);
        }
    }
    // 파일 하나가 실패했을 수도 있지만(권한 등), 명령 자체가 없는 쪽이 훨씬 흔하다.
    // 잘못 기억해도 손해는 "이 연결에서 검증을 건너뛴다"이고, 그 사실은 화면에 남는다.
    probe.set(false);
    SKIPPED.fetch_add(1, Ordering::Relaxed);
    None
}

/// 명령 하나를 exec 채널로 실행해 성공(exit 0) 시 출력에서 해시를 뽑는다.
async fn exec_hash(handle: &Handle<Handler>, cmd: &str) -> Option<String> {
    let mut ch = handle.channel_open_session().await.ok()?;
    ch.exec(true, cmd).await.ok()?;
    let (mut out, mut code) = (Vec::new(), None);
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
            _ => {}
        }
        if out.len() > 4096 {
            break; // 해시 한 줄이면 충분 — 폭주 방어.
        }
    }
    (code == Some(0)).then(|| parse_hash_output(&String::from_utf8_lossy(&out))).flatten()
}

/// 로컬 파일의 SHA-256(스트리밍 — 메모리에 다 올리지 않는다).
pub(crate) fn local_sha256(path: &str) -> Result<String, String> {
    use sha2::Digest;
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    // digest 0.11의 출력 배열은 LowerHex를 구현하지 않는다 — 바이트를 직접 hex로.
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_hostile_paths() {
        assert_eq!(shell_quote("/tmp/a.txt"), "'/tmp/a.txt'");
        // 단일 인용부호가 든 경로도 주입이 되지 않는 형태로 감싼다.
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    }

    #[test]
    fn parses_hash_lines() {
        let h = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(parse_hash_output(&format!("{h}  /tmp/f.bin\n")).as_deref(), Some(h));
        assert_eq!(parse_hash_output(&format!("{}  f", h.to_uppercase())).as_deref(), Some(h), "대문자 출력도 소문자로 정규화");
        assert_eq!(parse_hash_output("sha256sum: missing operand"), None);
        assert_eq!(parse_hash_output(""), None);
        // 32자리(md5)는 sha256이 아니다 — 잘못 매칭하지 않는다.
        assert_eq!(parse_hash_output("d41d8cd98f00b204e9800998ecf8427e  f"), None);
    }

    #[test]
    fn local_hash_matches_known_vector() {
        let dir = std::env::temp_dir().join(format!("nabi-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("v.bin");
        std::fs::write(&p, b"abc").unwrap();
        // SHA-256("abc") 표준 테스트 벡터.
        assert_eq!(
            local_sha256(p.to_str().unwrap()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_file(&p);
    }
    #[test]
    fn a_probe_starts_unknown_and_remembers_both_answers() {
        // 답은 연결이 사는 동안 바뀌지 않는다. 한 번 알아내면 다시 묻지 않는다 —
        // 예전에는 명령이 없는 서버에서 파일마다 exec 채널 두 개를 열고 두 번 다 실패했다.
        let p = HashProbe::default();
        assert_eq!(p.known(), None, "처음엔 모른다");
        p.set(false);
        assert_eq!(p.known(), Some(false), "없다고 기억한다");
        let q = HashProbe::default();
        q.set(true);
        assert_eq!(q.known(), Some(true));
    }

    #[test]
    fn the_tally_separates_verified_from_skipped() {
        // 구분이 안 되면 검증을 켜 둔 사용자는 아무것도 검증되지 않은 전송을 검증된 것으로
        // 믿는다 — 신뢰가 전부인 기능에서 그것이 가장 나쁜 실패다.
        let before = tally();
        VERIFIED.fetch_add(2, Ordering::Relaxed);
        SKIPPED.fetch_add(3, Ordering::Relaxed);
        let after = tally();
        assert_eq!(after.0 - before.0, 2);
        assert_eq!(after.1 - before.1, 3);
    }

}
