//! nabi-control — 에이전트 제어 평면(docs/agent-control.md).
//!
//! pane 안의 프로세스가 named pipe(NDJSON)로 nabiTerm을 질의·제어한다.
//! CP-1: 읽기 전용(list/capture) + 토큰 검증. 쓰기 동작은 CP-2.

pub mod discovery;
pub mod client;
mod clientagent;
mod clientverbs;
mod dispatchread;
pub mod apidoc;
pub mod keyspec;
pub mod integration;
mod explain;
pub mod dispatch;
pub mod matcher;
pub mod mcp;
pub mod pipe_acl;
pub mod policy;
mod gate;
pub mod trail;
pub mod protocol;
pub mod server;
pub mod subscribe;
pub mod tail_delta;

/// 현재 프로세스의 제어 파이프 이름(자기 인스턴스 스코프).
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\nabi-control-{}", std::process::id())
}

/// 물려받은 파이프 이름이 **다른 nabiTerm의 것**인가.
///
/// 환경 변수는 자식 셸이 부모 앱을 찾으라고 있는 것이다. 그런데 나비텀 **안의** 셸에서
/// 나비텀을 또 실행하면 그 값이 그대로 딸려 온다 — 그러면 새 앱이 자기 서버가 아니라
/// 부모의 주소를 자기 것인 양 기록해, 탐색기 '여기서 열기'가 엉뚱한 곳을 두드린다
/// (2026-08-25에 실제로 그렇게 됐다).
///
/// 판정은 이름 규칙으로 한다 — `nabi-control-<pid>` 꼴인데 그 pid가 내가 아니면 남의 것이다.
/// 시험 하네스가 일부러 넣는 다른 이름(`nabi-e2e-…`)은 규칙에 안 맞으므로 그대로 존중한다.
pub fn is_foreign_pipe(name: &str) -> bool {
    let Some((_, tail)) = name.rsplit_once("nabi-control-") else {
        return false;
    };
    match tail.parse::<u32>() {
        Ok(pid) => pid != std::process::id(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod pipe_tests {
    use super::*;

    #[test]
    fn our_own_pipe_is_not_foreign() {
        assert!(!is_foreign_pipe(&pipe_name()));
    }

    /// 나비텀 안에서 나비텀을 또 띄운 경우 — 부모 것을 자기 것으로 쓰면 안 된다.
    #[test]
    fn another_instances_pipe_is_foreign() {
        let other = format!(r"\\.\pipe\nabi-control-{}", std::process::id() + 1);
        assert!(is_foreign_pipe(&other));
    }

    /// 시험 하네스가 일부러 정해 준 이름은 존중한다(규칙에 안 맞으므로 남의 것이 아니다).
    #[test]
    fn a_deliberate_test_name_is_respected() {
        assert!(!is_foreign_pipe(r"\\.\pipe\nabi-e2e-1234"));
        assert!(!is_foreign_pipe(""));
    }
}

/// 접속 토큰 생성 — OS CSPRNG에서 얻은 256-bit 비예측 값.
/// (1차 방어는 사용자 세션 스코프 파이프, 토큰은 2차 방어.)
pub fn gen_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tokens_are_full_length_and_unique() {
        let a = super::gen_token();
        let b = super::gen_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
