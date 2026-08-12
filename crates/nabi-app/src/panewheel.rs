//! 휠 한 번이 어디로 가는지 정하는 규칙과, 앱에 보낼 키 시퀀스.
//!
//! 탭(`tabsterm`)과 분리 창(`floatterm`)이 같은 함수를 쓴다 — 규칙이 갈라지면 창을 뗐을 때
//! 동작이 달라진다. 화면 상태(대체 화면·마우스 보고·DEC 1007)와 사용자 설정을 한곳에서 본다.

/// 휠을 어디로 보낼지 정하는 데 필요한 화면 상태와 사용자 설정.
#[derive(Clone, Copy, Default)]
pub(crate) struct WheelCtx {
    /// 대체 화면(vim·less) — 스크롤백이 없다.
    pub alt_screen: bool,
    /// 앱이 DEC 1007로 "휠을 커서 키로" 요청했다.
    pub alt_scroll: bool,
    /// 앱이 마우스 보고를 켜서 휠을 직접 받는다.
    pub mouse_on: bool,
    /// 사용자가 이 pane에 "휠을 키로 보내기"를 켰다(스크롤백을 안 남기는 TUI 대응).
    pub force_keys: bool,
    /// Shift를 누른 채 굴렸다.
    pub shift: bool,
}

/// 휠 한 번이 무엇을 움직이는가.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WheelTo {
    /// 우리 스크롤백(보통의 셸).
    Scrollback,
    /// 앱에 PageUp/PageDown.
    PageKeys,
    /// 앱에 커서 키(DEC 1007 규격).
    CursorKeys,
    /// 아무것도 하지 않는다(앱이 이미 마우스 보고로 받았다).
    Nothing,
}

/// 화면 상태와 설정을 보고 휠의 목적지를 고른다.
///
/// 규칙을 한곳에 모아 둔다 — 탭과 분리 창이 서로 다르게 굴면 창을 뗐을 때 동작이 바뀐다.
pub(crate) fn wheel_target(c: WheelCtx) -> WheelTo {
    if c.mouse_on {
        // 앱이 휠을 직접 받는다. 앱이 원하는 상황(대체 화면·1007)이나 Shift면 겹치지 않는다.
        return match c.alt_screen || c.alt_scroll || c.shift {
            true => WheelTo::Nothing,
            false => WheelTo::Scrollback, // 주 화면에서는 스크롤백이 우선.
        };
    }
    if c.alt_screen {
        return WheelTo::PageKeys; // 스크롤백이 없으니 Shift라도 앱에 넘긴다.
    }
    // Shift는 앱을 건너뛰고 스크롤백을 보는 길 — 주 화면에서만 뜻이 있다.
    if c.shift {
        return match c.alt_scroll || c.force_keys {
            true => WheelTo::Scrollback,
            false => WheelTo::Nothing,
        };
    }
    match (c.alt_scroll, c.force_keys) {
        (true, _) => WheelTo::CursorKeys,
        (false, true) => WheelTo::PageKeys,
        (false, false) => WheelTo::Scrollback,
    }
}

/// alternate scroll(DEC 1007): 휠을 커서 위/아래 키로 바꾼다. 한 눈금에 3줄(xterm 관례).
///
/// 앱 커서 키 모드(DECCKM)면 `ESC O A/B`, 아니면 `ESC [ A/B`다 — 모드를 무시하면 TUI가
/// 키를 못 알아듣는다.
pub(crate) fn alt_scroll_bytes(wheel: f32, app_cursor: bool) -> Vec<u8> {
    if wheel == 0.0 {
        return Vec::new();
    }
    let key: &[u8] = match (wheel.is_sign_positive(), app_cursor) {
        (true, true) => b"\x1bOA",
        (true, false) => b"\x1b[A",
        (false, true) => b"\x1bOB",
        (false, false) => b"\x1b[B",
    };
    key.repeat(3)
}

/// 목적지에 맞는 키 시퀀스(스크롤백으로 갈 때는 None — 보낼 바이트가 없다).
pub(crate) fn wheel_bytes(target: WheelTo, wheel: f32, app_cursor: bool) -> Option<Vec<u8>> {
    match target {
        WheelTo::CursorKeys => Some(alt_scroll_bytes(wheel, app_cursor)),
        WheelTo::PageKeys => Some(tui_scroll_bytes(wheel)),
        WheelTo::Scrollback | WheelTo::Nothing => None,
    }
}

/// 마우스를 받지 않는 대체 화면 TUI에서 휠을 PageUp/PageDown으로 바꾼다.
pub(crate) fn tui_scroll_bytes(wheel: f32) -> Vec<u8> {
    if wheel == 0.0 {
        return Vec::new();
    }
    if wheel.is_sign_positive() { b"\x1b[5~".to_vec() } else { b"\x1b[6~".to_vec() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 보통의 셸: 휠은 스크롤백. 앱에 키를 보내지 않는다.
    #[test]
    fn plain_shell_scrolls_scrollback() {
        assert_eq!(wheel_target(WheelCtx::default()), WheelTo::Scrollback);
    }

    /// 대체 화면은 스크롤백이 없다 — Shift를 눌러도 빠져나갈 곳이 없으니 앱에 넘긴다.
    #[test]
    fn alt_screen_always_goes_to_app() {
        let c = WheelCtx { alt_screen: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::PageKeys);
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::PageKeys);
    }

    /// DEC 1007을 켠 앱은 커서 키로, Shift면 우리 스크롤백으로 빠진다.
    #[test]
    fn dec1007_uses_cursor_keys_with_shift_escape() {
        let c = WheelCtx { alt_scroll: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::CursorKeys);
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::Scrollback);
    }

    /// 사용자가 켠 pane은 PageUp/PageDown으로, Shift면 스크롤백으로.
    #[test]
    fn user_forced_keys_use_page_keys() {
        let c = WheelCtx { force_keys: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::PageKeys);
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::Scrollback);
    }

    /// 마우스 보고를 켠 앱에는 우리가 겹쳐 움직이지 않는다(이중 스크롤 방지).
    #[test]
    fn mouse_reporting_app_is_not_doubled() {
        let c = WheelCtx { mouse_on: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::Scrollback); // 주 화면은 스크롤백이 우선.
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::Nothing);
        assert_eq!(wheel_target(WheelCtx { alt_screen: true, ..c }), WheelTo::Nothing);
        // 마우스 보고와 1007을 함께 켠 앱은 마우스 보고 쪽이 이긴다(주류 에뮬레이터와 같게).
        assert_eq!(wheel_target(WheelCtx { alt_scroll: true, ..c }), WheelTo::Nothing);
    }

    #[test]
    fn tui_scroll_uses_page_keys() {
        assert_eq!(tui_scroll_bytes(1.0), b"\x1b[5~");
        assert_eq!(tui_scroll_bytes(-1.0), b"\x1b[6~");
    }

    /// DEC 1007은 커서 키로 보낸다 — 앱 커서 모드(DECCKM)면 `ESC O`, 아니면 `ESC [`.
    #[test]
    fn alt_scroll_uses_cursor_keys() {
        assert_eq!(alt_scroll_bytes(1.0, false), b"\x1b[A\x1b[A\x1b[A".to_vec());
        assert_eq!(alt_scroll_bytes(-1.0, false), b"\x1b[B\x1b[B\x1b[B".to_vec());
        assert_eq!(alt_scroll_bytes(1.0, true), b"\x1bOA\x1bOA\x1bOA".to_vec());
        assert!(alt_scroll_bytes(0.0, false).is_empty());
    }
}
