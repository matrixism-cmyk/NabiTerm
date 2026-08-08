//! 차단형 프롬프트가 뜨면 메인 창을 앞으로 불러온다.
//!
//! 호스트키 확인·제어 승인·재접속 같은 모달은 **메인 창에서만** 그려진다. 그런데
//! 분리 창(별도 OS 창)에서 일하는 중에도 뜰 수 있다 — 분리 터미널의 SSH가 끊기거나,
//! 처음 보는 서버에 붙거나, 에이전트가 권한을 요청할 때다. 그러면 사용자는 아무 일도
//! 일어나지 않는 화면을 보고 있고, 연결은 대답을 기다리며 멈춰 있다(호스트키 확인은
//! 최대 180초). 특히 호스트키는 지문을 확인해야 하는 **보안** 프롬프트라 놓치면 안 된다.
//!
//! 모달을 창마다 복제하는 대신 메인 창을 앞으로 부른다 — 프롬프트마다 "어느 창에
//! 속하는지"가 분명하지 않은 것들(제어 승인 등)까지 한 번에 덮인다.

/// 이번 프레임에 메인 창을 앞으로 불러야 하는가.
///
/// **떠오르는 순간 한 번만** 부른다. 매 프레임 부르면 사용자가 다른 창으로 옮길 수조차
/// 없고(포커스를 계속 빼앗김), 분리 창이 없으면 애초에 부를 이유도 없다.
pub(crate) fn should_raise(pending: bool, already: bool, has_detached: bool) -> bool {
    pending && !already && has_detached
}

#[cfg(test)]
mod tests {
    use super::should_raise;

    #[test]
    fn raises_once_on_appearance() {
        assert!(should_raise(true, false, true), "떠오르는 순간 부른다");
        assert!(!should_raise(true, true, true), "이미 불렀으면 다시 안 부른다");
    }

    /// 분리 창이 없으면 메인 창이 곧 사용자가 보는 창이다 — 건드릴 이유가 없다.
    #[test]
    fn does_nothing_without_detached_windows() {
        assert!(!should_raise(true, false, false));
    }

    #[test]
    fn no_prompt_no_raise() {
        assert!(!should_raise(false, false, true));
        assert!(!should_raise(false, true, true));
    }
}
