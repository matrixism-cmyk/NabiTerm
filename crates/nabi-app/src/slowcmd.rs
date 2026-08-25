//! **명령이 끝났다고 알릴지** 정한다.
//!
//! 알림은 이미 있었지만 조건이 좁았다(`events.rs`).
//!
//! ```text
//! 예전:  그 pane이 포커스가 아니고 && (실패 || 10초 이상)
//! ```
//!
//! 두 가지가 걸린다.
//!
//! 1. **창이 뒤에 있을 때를 놓친다.** 빌드를 걸어 놓고 브라우저로 넘어가는 것이 가장 흔한
//!    경우인데, 그때 pane은 여전히 포커스라서 조건에 걸리지 않았다. 정작 알림이 가장
//!    필요한 상황이다.
//! 2. **10초가 코드에 박혀 있었다.** 사람마다 "오래"가 다르다. 빌드가 늘 3분인 사람에게
//!    10초 알림은 소음이고, 배포 스크립트가 8초인 사람에게는 놓치는 값이다.
//!
//! 알림은 **놓치는 것보다 성가신 것이 더 나쁘다.** 눈앞에 있는 창에서 방금 친 명령이
//! 끝난 것을 굳이 알려 주면 신뢰를 잃고, 그때부터는 진짜 알림도 무시하게 된다. 그래서
//! 보고 있는 자리에서는 절대 울리지 않는다.

/// 알림을 낼 것인가.
///
/// * `pane_focused` — 그 pane이 지금 보고 있는 탭인가.
/// * `window_focused` — nabiTerm 창 자체가 앞에 있는가.
/// * `failed` — 종료 코드가 0이 아닌가.
/// * `secs` — 걸린 시간(초).
/// * `threshold` — 설정값. **0이면 시간 기준 알림을 끈다**(실패 알림은 남는다).
pub(crate) fn should_notify(
    pane_focused: bool,
    window_focused: bool,
    failed: bool,
    secs: u64,
    threshold: u64,
) -> bool {
    // 보고 있는 자리면 알리지 않는다 — 방금 친 명령이 끝난 것은 화면에 이미 있다.
    if pane_focused && window_focused {
        return false;
    }
    // 실패는 시간과 무관하게 알린다. 짧게 실패한 것이 오히려 놓치기 쉽다.
    failed || (threshold > 0 && secs >= threshold)
}

#[cfg(test)]
mod tests {
    use super::should_notify;

    /// **가장 흔한 경우** — 빌드를 걸어 두고 다른 창으로 넘어갔다. 예전에는 여기서 조용했다.
    #[test]
    fn a_long_command_notifies_when_the_window_is_in_the_background() {
        assert!(should_notify(true, false, false, 60, 30));
    }

    /// 보고 있는 자리에서는 울리지 않는다 — 화면에 이미 결과가 있다.
    #[test]
    fn nothing_fires_while_you_are_watching_that_pane() {
        assert!(!should_notify(true, true, false, 600, 30));
        assert!(!should_notify(true, true, true, 600, 30), "실패해도 보고 있으면 조용하다");
    }

    /// 다른 탭에서 끝났으면 창이 앞에 있어도 알린다 — 그 탭은 보이지 않는다.
    #[test]
    fn another_tab_finishing_still_notifies() {
        assert!(should_notify(false, true, false, 60, 30));
    }

    /// **실패는 시간과 무관하다.** 1초 만에 죽은 것이 오히려 눈에 안 띈다.
    #[test]
    fn a_failure_notifies_however_short_it_was() {
        assert!(should_notify(false, false, true, 0, 30));
        assert!(should_notify(false, false, true, 1, 3600));
    }

    /// 임계값 아래의 성공은 소음이다.
    #[test]
    fn a_quick_success_stays_quiet() {
        assert!(!should_notify(false, false, false, 5, 30));
    }

    /// 경계에서 정확히 울린다(29초는 조용, 30초는 알림).
    #[test]
    fn the_threshold_is_inclusive() {
        assert!(!should_notify(false, false, false, 29, 30));
        assert!(should_notify(false, false, false, 30, 30));
    }

    /// **0이면 시간 알림을 끈다** — 다만 실패는 계속 알린다. 끄고 싶은 것은 소음이지
    /// 사고 소식이 아니다.
    #[test]
    fn zero_turns_off_the_time_rule_but_not_failures() {
        assert!(!should_notify(false, false, false, 9999, 0));
        assert!(should_notify(false, false, true, 0, 0));
    }
}
