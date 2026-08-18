//! 텔레그램 브리지 설정(schema.rs에서 분리 — 라인 한도).

use serde::{Deserialize, Serialize};

/// 텔레그램 브리지 설정(봇 토큰은 keyring에 별도 저장 — 여기엔 비밀 미포함).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramCfg {
    /// 브리지 마스터 on/off(끄면 폴링 안 함).
    pub enabled: bool,
    /// 허용 chat ID 화이트리스트(비면 모두 거부 — 보안).
    pub allowed_chats: Vec<i64>,
    /// 텔레그램에 모든 권한 부여(목록/캡처/입력/스폰 전부 — 화이트리스트 chat 한정).
    pub grant_all: bool,
    /// 회신에 포함할 출력 줄 수.
    pub reply_lines: usize,
    /// 명령 후 출력 대기 한도(ms).
    pub idle_timeout_ms: u64,
    /// getUpdates 롱폴링 timeout(초).
    pub poll_secs: u64,
    /// 미지 DM 처리: "allowlist"(무시, 기존 동작) | "pairing"(만료 코드 발급→앱에서 승인).
    /// OpenClaw DM pairing 벤치마킹(C1). open(전체 허용)은 만들지 않는다 — 사고 벡터.
    pub dm_policy: String,
    /// 하트비트 주기(분, 0=끄기) — 에이전트 상태 요약을 오너 chat에 발신(C5).
    /// 변화가 없으면 발신하지 않는다(OpenClaw HEARTBEAT_OK 억제 패턴 — 스팸·비용 방지).
    pub heartbeat_mins: u64,
}

impl Default for TelegramCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_chats: Vec::new(),
            grant_all: false,
            reply_lines: 40,
            dm_policy: "allowlist".into(),
            heartbeat_mins: 0,
            idle_timeout_ms: 8000,
            poll_secs: 30,
        }
    }
}
