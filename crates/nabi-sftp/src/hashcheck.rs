//! 전송 무결성 해시 검증(T2-1, rclone 방식) — 크기 비교 위의 한 단계.
//!
//! rclone처럼 폴백 사다리를 탄다: ① 원격에서 `sha256sum` 계열 명령을 실행해 해시를 얻고
//! 로컬 해시와 대조 ② 명령이 없는 서버(Windows OpenSSH 등)면 조용히 건너뛴다 —
//! 크기 비교(xfer.rs)는 항상 수행되므로 최소선은 유지된다.
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

pub(crate) fn enabled() -> bool {
    SFTP_VERIFY_HASH.load(Ordering::Relaxed)
}

/// POSIX 셸 단일 인용 — 경로에 어떤 문자가 있어도 명령 주입이 되지 않게.
pub(crate) fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
}

/// `sha256sum`/`shasum` 출력에서 64자리 16진 해시만 뽑는다(형식: `<hex>  <path>`).
pub(crate) fn parse_hash_output(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|t| t.to_ascii_lowercase())
}

/// 원격 파일의 SHA-256을 셸 명령으로 얻는다. 명령이 없거나 실패하면 None(폴백 사다리).
pub(crate) async fn remote_sha256(handle: &Handle<Handler>, path: &str) -> Option<String> {
    let q = shell_quote(path);
    // GNU coreutils → BSD/macOS 순으로 시도.
    for cmd in [format!("sha256sum -- {q}"), format!("shasum -a 256 -- {q}")] {
        if let Some(h) = exec_hash(handle, &cmd).await {
            return Some(h);
        }
    }
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
}
