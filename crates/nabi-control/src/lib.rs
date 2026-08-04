//! nabi-control — 에이전트 제어 평면(docs/agent-control.md).
//!
//! pane 안의 프로세스가 named pipe(NDJSON)로 nabiTerm을 질의·제어한다.
//! CP-1: 읽기 전용(list/capture) + 토큰 검증. 쓰기 동작은 CP-2.

pub mod client;
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

/// 접속 토큰 생성 — 시각·PID·스택 주소를 섞은 비예측 헥사 문자열.
/// (1차 방어는 사용자 세션 스코프 파이프, 토큰은 2차 방어.)
pub fn gen_token() -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .hash(&mut h);
    std::process::id().hash(&mut h);
    let probe = 0u8; // ASLR 스택 주소를 엔트로피에 추가.
    (&probe as *const u8 as usize).hash(&mut h);
    format!("{:016x}", h.finish())
}
