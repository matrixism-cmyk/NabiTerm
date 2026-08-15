//! nabi-control — 에이전트 제어 평면(docs/agent-control.md).
//!
//! pane 안의 프로세스가 named pipe(NDJSON)로 nabiTerm을 질의·제어한다.
//! CP-1: 읽기 전용(list/capture) + 토큰 검증. 쓰기 동작은 CP-2.

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
pub mod protocol;
pub mod server;
pub mod subscribe;
pub mod tail_delta;

/// 현재 프로세스의 제어 파이프 이름(자기 인스턴스 스코프).
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\nabi-control-{}", std::process::id())
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
