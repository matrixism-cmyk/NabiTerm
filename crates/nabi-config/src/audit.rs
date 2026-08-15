//! 보안 감사(C6, 순수) — 설정값을 점검해 위험한 조합을 checkId+심각도로 보고한다.
//!
//! OpenClaw의 `security audit`를 벤치마킹하되 **보고 전용**이다(자동 수정은 설정을 몰래
//! 바꾸는 또 다른 위험 — 각 항목에 "어디서 고치는지"를 적는 쪽을 택했다).
//! nabiTerm은 named pipe(로컬 전용)라 네트워크 노출 계열 사고(OpenClaw 포트 개방 13만+)는
//! 구조적으로 없다 — 감사는 로컬 권한 위임 항목에 집중한다.

use crate::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 알아두면 좋은 절충(의도했을 수 있음).
    Info,
    /// 권한이 넓게 열려 있음 — 의도 확인 권장.
    Warn,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: &'static str,
    pub severity: Severity,
    pub message: String,
    /// 어디서 고치는가.
    pub fix_at: &'static str,
}

/// 설정을 감사한다. 비어 있으면 "특이사항 없음".
pub fn audit(cfg: &AppConfig) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut f = |id, severity, message: String, fix_at| out.push(Finding { id, severity, message, fix_at });

    if cfg.terminal.control_mode == "on" {
        f("control-mode-on", Severity::Warn,
          "에이전트 제어가 무승인(on) — pane 안의 어떤 프로세스든 셸 주입·종료가 가능합니다".into(),
          "설정 ▸ 동작 ▸ 에이전트 제어");
    }
    if cfg.terminal.control_allow_osc {
        f("osc-control-on", Severity::Warn,
          "OSC 7771 in-band 제어 허용 — 터미널에 출력할 수 있는 쪽이면 제어를 시도할 수 있습니다".into(),
          "설정 ▸ 동작");
    }
    if cfg.terminal.osc52_mode == 2 {
        f("osc52-silent", Severity::Warn,
          "원격 클립보드 쓰기(OSC 52)가 무알림 허용 — 원격이 클립보드를 몰래 바꿔도 모릅니다".into(),
          "설정 ▸ 터미널 ▸ 원격 클립보드");
    }
    if cfg.telegram.enabled {
        if cfg.telegram.grant_all {
            f("tg-grant-all", Severity::Warn,
              "텔레그램 '모든 권한 부여' 켜짐 — 오너 chat이 원격에서 셸을 제어합니다(폰 분실 시 위험)".into(),
              "설정 ▸ 텔레그램");
        }
        if cfg.telegram.dm_policy == "pairing" {
            f("tg-pairing", Severity::Info,
              "텔레그램 페어링 모드 — 미지 DM이 승인 요청을 만들 수 있습니다(상한 3건)".into(),
              "설정 ▸ 텔레그램 ▸ 미지 DM 처리");
        }
        if cfg.telegram.allowed_chats.is_empty() {
            f("tg-no-chats", Severity::Info,
              "텔레그램이 켜져 있지만 허용 chat이 없어 동작하지 않습니다".into(),
              "설정 ▸ 텔레그램");
        }
    }
    if cfg.terminal.vault_remember {
        f("vault-remember", Severity::Info,
          "볼트 자동 잠금해제 — 같은 OS 계정의 다른 프로그램도 볼트를 열 수 있는 절충입니다".into(),
          "설정 ▸ 동작");
    }
    if cfg.terminal.restore_ssh_ai_command {
        f("ssh-ai-restore", Severity::Info,
          "SSH 복원 시 AI CLI 자동 재실행 — 원격 서버에서 명령이 자동 실행됩니다(허용 목록 한정)".into(),
          "설정 ▸ 터미널");
    }
    if cfg.terminal.ai_cli_auto_update {
        f("ai-auto-update", Severity::Info,
          "AI CLI 자동 업데이트 — 서드파티 프로그램을 자동으로 갈아 끼웁니다(npm/공식 채널)".into(),
          "도움말 ▸ AI 제어");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 기본 설정은 경고가 없어야 한다 — 기본값이 안전하다는 회귀 가드.
    #[test]
    fn defaults_are_quiet() {
        let cfg = AppConfig::default();
        let warns: Vec<_> = audit(&cfg).into_iter().filter(|f| f.severity == Severity::Warn).collect();
        assert!(warns.is_empty(), "기본값에서 경고: {warns:?}");
    }

    #[test]
    fn wide_grants_are_flagged() {
        let mut cfg = AppConfig::default();
        cfg.terminal.control_mode = "on".into();
        cfg.terminal.osc52_mode = 2;
        cfg.telegram.enabled = true;
        cfg.telegram.grant_all = true;
        let ids: Vec<_> = audit(&cfg).iter().map(|f| f.id).collect();
        assert!(ids.contains(&"control-mode-on"));
        assert!(ids.contains(&"osc52-silent"));
        assert!(ids.contains(&"tg-grant-all"));
    }
}
