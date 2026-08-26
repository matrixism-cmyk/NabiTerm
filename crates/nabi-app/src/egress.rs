//! **이 프로그램이 바깥으로 나가는 곳** — 목록과, 나갈지 말지를 정하는 한 곳.
//!
//! 우리는 폐쇄망을 차별화로 내걸었는데 정작 그 모드가 없었다. 그러는 동안 프로그램은
//! **묻지 않고** 밖으로 나갔다 — 켤 때마다 공인 IP를 조회하고, 주기적으로 새 판을 확인했다.
//!
//! 폐쇄망(정부·금융·공장)에서는 그런 호출이 보안 경보를 띄우고 프록시 로그에 남는다.
//! 폐쇄망이 아니어도, 공인 IP 조회는 **내 IP를 제삼자에게 알려 주는 일**이다.
//!
//! ## 왜 목록을 코드에 두나
//!
//! "이 프로그램이 어디에 연결합니까?"는 보안 검토에서 반드시 나오는 질문이다. 그 답이
//! 문서에만 있으면 **코드가 바뀔 때 조용히 틀린 답이 된다.** 그래서 목록을 코드에 두고,
//! 도움말이 그것을 읽어 보여 준다. 새 호출을 넣는 사람은 이 표에 한 줄을 더해야 한다.
//!
//! ## 무엇을 막고 무엇을 안 막나
//!
//! 오프라인 모드가 막는 것은 **사용자가 시키지 않은** 호출뿐이다. 글꼴 내려받기나 AI CLI
//! 설치는 사용자가 단추를 눌러 시작한 일이므로 막지 않는다 — 눌렀는데 아무 일도 없으면
//! 그건 보호가 아니라 고장이다.

/// 바깥으로 나가는 곳 하나.
pub(crate) struct Egress {
    /// 어디로.
    pub host: &'static str,
    /// 무엇 때문에(i18n 키).
    pub why: &'static str,
    /// 사용자가 시키지 않아도 나가는가 — 오프라인 모드가 막는 것이 이것들이다.
    pub unattended: bool,
}

/// **전체 목록.** 새 호출을 넣으면 여기에도 한 줄 더한다.
pub(crate) const ALL: &[Egress] = &[
    Egress { host: "api.ipify.org", why: "egress.why.ip", unattended: true },
    Egress { host: "api.github.com", why: "egress.why.update", unattended: true },
    Egress { host: "github.com", why: "egress.why.download", unattended: false },
    Egress { host: "fonts.google.com", why: "egress.why.font", unattended: false },
    Egress { host: "registry.npmjs.org", why: "egress.why.aicli", unattended: false },
    Egress { host: "pypi.org", why: "egress.why.aicli", unattended: false },
];

/// 지금 오프라인 모드인가 — 설정에서 정하고, 나가려는 쪽이 이 값을 본다.
///
/// 전역으로 둔 까닭: 나가는 자리가 여러 크레이트에 흩어져 있고, 그 각각에 설정을 들려
/// 보내면 언젠가 한 곳이 빠진다. keepalive·팔레트와 같은 방식이다.
pub(crate) static OFFLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 설정이 바뀔 때 부른다.
pub(crate) fn set_offline(on: bool) {
    OFFLINE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// 시키지 않은 호출을 지금 해도 되나.
///
/// 사용자가 눌러서 시작한 일에는 **묻지 않는다**(`unattended: false`) — 눌렀는데 아무 일도
/// 없으면 그건 보호가 아니라 고장이다.
pub(crate) fn may_call_unattended() -> bool {
    !OFFLINE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::{may_call_unattended, set_offline, ALL};

    /// 스위치가 스위치 노릇을 한다.
    #[test]
    fn the_switch_actually_switches() {
        set_offline(true);
        assert!(!may_call_unattended());
        set_offline(false);
        assert!(may_call_unattended(), "끄면 다시 나갈 수 있어야 한다");
    }

    /// 기본은 **꺼짐** — 지금 쓰는 사람의 동작을 바꾸지 않는다.
    ///
    /// 전역 하나를 여러 시험이 함께 쓰므로 값을 직접 보지 않고 **설정 기본값**을 본다.
    /// (전역의 초기값은 다른 시험이 이미 바꿔 놓았을 수 있다.)
    #[test]
    fn the_default_is_online() {
        assert!(!nabi_config::AppConfig::default().terminal.offline_mode);
    }

    /// 목록이 비어 있으면 도움말이 거짓말을 한다.
    #[test]
    fn the_list_is_not_empty_and_every_entry_is_filled() {
        assert!(ALL.len() >= 4);
        for e in ALL {
            assert!(!e.host.is_empty(), "호스트가 비었다");
            assert!(e.why.starts_with("egress.why."), "사유 키가 규칙에서 벗어났다: {}", e.why);
        }
    }

    /// **시키지 않은 호출이 실제로 있다** — 없다면 이 배치가 필요 없다.
    #[test]
    fn some_calls_really_are_unattended() {
        let n = ALL.iter().filter(|e| e.unattended).count();
        assert!(n >= 2, "묻지 않고 나가는 곳이 없다면 스위치도 필요 없다");
    }

    /// 사용자가 눌러 시작하는 것은 막지 않는다 — 목록에 그런 것이 있어야 뜻이 산다.
    #[test]
    fn user_started_calls_exist_and_are_not_blocked() {
        assert!(ALL.iter().any(|e| !e.unattended));
    }
}
