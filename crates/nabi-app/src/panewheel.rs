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
    /// TUI 기록 오버레이(codex Ctrl+T)가 열려 있다고 우리가 추적 중인가.
    pub overlay: bool,
    /// 휠 방향이 위쪽인가(과거를 보려는 것).
    pub up: bool,
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
    /// TUI 기록 오버레이를 연다(codex Ctrl+T) — 열린 뒤의 휠은 PageKeys로 흐른다.
    OpenTui,
    /// 아무것도 하지 않는다(앱이 이미 마우스 보고로 받았다).
    Nothing,
}

/// 화면 상태와 설정을 보고 휠의 목적지를 고른다.
///
/// 규칙을 한곳에 모아 둔다 — 탭과 분리 창이 서로 다르게 굴면 창을 뗐을 때 동작이 바뀐다.
pub(crate) fn wheel_target(c: WheelCtx) -> WheelTo {
    // **DEC 1007은 대체 화면에서만 뜻이 있다.** xterm 규격이 그렇게 정의한다 —
    // "Alternate Scroll 모드가 켜져 있으면 터미널이 *대체 화면을 표시하고 있을 때*
    // 커서 위/아래를 보낸다."
    //
    // 우리는 그 조건을 빼먹고 주 화면에서도 1007을 따랐다. 그래서 마우스 보고와 1007을
    // 함께 켜는 TUI(Claude Code 등)를 주 화면에서 쓰면 휠이 아무 일도 하지 않았다 —
    // 스크롤백은 멀쩡히 쌓여 있는데 볼 방법이 없었다(사용자 보고 2026-08-25).
    // 주 화면에는 진짜 스크롤백이 있고, 거기서 휠은 그것을 보는 도구다.
    let c = WheelCtx { alt_scroll: c.alt_scroll && c.alt_screen, ..c };
    if c.mouse_on {
        // 앱이 휠을 직접 받는다. 대체 화면이거나 Shift면 우리가 겹쳐 움직이지 않는다.
        return match c.alt_screen || c.shift {
            true => WheelTo::Nothing,
            false => WheelTo::Scrollback, // 주 화면에서는 스크롤백이 우선.
        };
    }
    // 1007을 **대체 화면 판정보다 먼저** 본다. 뒤에 두면 아래 alt_screen 분기가 먼저
    // 걸려 커서 키가 영영 나가지 않는다(이 순서를 놓쳐 시험이 잡았다).
    if c.alt_scroll {
        return WheelTo::CursorKeys;
    }
    if c.alt_screen {
        return WheelTo::PageKeys; // 스크롤백이 없으니 Shift라도 앱에 넘긴다.
    }
    // 여기부터는 주 화면 — 스크롤백이 있다.
    if c.shift {
        return WheelTo::Scrollback; // Shift는 앱을 건너뛰고 우리 스크롤백을 보는 길.
    }
    // 기록을 자기 오버레이에만 두는 TUI(codex): 오버레이가 닫혀 있으면 위로 굴릴 때
    // 먼저 열어 준다(Ctrl+T). 이미 열려 있으면 페이지 키가 그 안에서 스크롤한다.
    // 아래로 굴리는데 오버레이도 없다면 볼 과거가 없다 — 보내지 않는다.
    match (c.force_keys, c.overlay, c.up) {
        (true, true, _) => WheelTo::PageKeys,
        (true, false, true) => WheelTo::OpenTui,
        (true, false, false) => WheelTo::Nothing,
        (false, _, _) => WheelTo::Scrollback,
    }
}

/// 실행 중 명령이 "기록을 자기 오버레이에만 두는 TUI"(현재 codex)인가.
///
/// 이런 pane은 토글 없이도 휠 도우미를 기본으로 켠다 — 앱 재시작으로 토글(메모리 전용)이
/// 초기화되면 사용자는 "그냥 안 된다"고 느낀다(실제 보고). 감지는 셸 통합(OSC 633;E)이
/// 준 명령의 첫 토큰으로 한다.
pub(crate) fn is_tui_history_app(cmd: &str) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("");
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first).to_ascii_lowercase();
    let base = base.trim_end_matches(".exe").trim_end_matches(".cmd").trim_end_matches(".bat");
    base == "codex"
}

impl crate::app::NabiApp {
    /// 이 pane에서 휠 도우미가 켜져 있는가 — 명시 켬 ∪ (codex 자동 감지 − 명시 끔).
    pub(crate) fn wheel_keys_effective(&self, pane: nabi_types::PaneId) -> bool {
        self.wheel_keys.contains(&pane)
            || (!self.wheel_keys_off.contains(&pane)
                && self.run_cmd.get(&pane).is_some_and(|c| is_tui_history_app(c)))
    }
}

/// TUI 기록 오버레이 토글 키(codex Ctrl+T).
pub(crate) const TUI_OVERLAY_KEY: u8 = 0x14;

/// 화면 하단 안내줄이 TUI 기록 오버레이(codex 전사 화면)의 것인가.
///
/// 키 입력으로 상태를 추적하는 방식은 접었다 — codex의 Esc는 닫기가 아니라 "이전 메시지
/// 편집"이라 추측이 어긋난다(실측). 오버레이는 하단에 키 힌트 줄을 항상 그리므로 그걸
/// 직접 읽는 쪽이 진실이다.
pub(crate) fn overlay_marker(bottom_text: &str) -> bool {
    bottom_text.contains("q to quit") || bottom_text.contains("to scroll")
}

/// 방금 오버레이를 열었는가(화면 반영 전 공백기) — 이 동안 Ctrl+T 재전송을 막는다.
/// PTY 왕복+재그리기가 보통 수십 ms지만, 느린 원격/무거운 프레임을 넉넉히 본다.
pub(crate) fn recently_opened(sent: Option<&std::time::Instant>) -> bool {
    sent.is_some_and(|t| t.elapsed().as_millis() < 700)
}

/// pane 화면을 읽어 오버레이 열림을 판정한다(래치 우선 — 방금 열었으면 화면 반영 전에도 참).
pub(crate) fn overlay_open(
    panes: &nabi_orchestrator::SharedPanes,
    pane: nabi_types::PaneId,
    sent: Option<&std::time::Instant>,
) -> bool {
    recently_opened(sent)
        || panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).map(|v| v.model.clone()))
            .and_then(|mo| mo.lock().ok().map(|md| overlay_marker(&md.visible_bottom_text(2))))
            .unwrap_or(false)
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
        WheelTo::OpenTui => Some(vec![TUI_OVERLAY_KEY]),
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

    /// DEC 1007을 켠 앱은 **대체 화면에서** 커서 키를 받는다.
    ///
    /// 대체 화면에는 스크롤백이 없으므로 Shift로 빠져나갈 곳도 없다 — 그때도 앱에 넘긴다.
    /// (예전에는 1007을 주 화면에서도 따랐고, 그래서 Shift가 탈출구였다. 이제 주 화면에서는
    /// 1007을 아예 무시하므로 탈출할 일이 없다.)
    #[test]
    fn dec1007_uses_cursor_keys_on_the_alternate_screen() {
        let c = WheelCtx { alt_scroll: true, alt_screen: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::CursorKeys);
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::CursorKeys);
    }

    /// **주 화면에서는 1007을 따르지 않는다** — xterm 규격이 대체 화면 한정으로 정의한다.
    ///
    /// 이걸 지키지 않으면 스크롤백이 멀쩡히 쌓여 있는데도 휠이 그것을 못 본다.
    /// Claude Code처럼 마우스 보고와 1007을 함께 켜는 TUI에서 실제로 그랬다(사용자 보고).
    #[test]
    fn dec1007_is_ignored_on_the_primary_screen() {
        // 1007만 켠 경우 — 커서 키가 아니라 우리 스크롤백.
        let c = WheelCtx { alt_scroll: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::Scrollback);
        // 마우스 보고까지 켠 경우 — 예전에는 아무 일도 하지 않았다(휠이 죽었다).
        let both = WheelCtx { alt_scroll: true, mouse_on: true, ..Default::default() };
        assert_eq!(wheel_target(both), WheelTo::Scrollback);
        // 대체 화면으로 넘어가면 그때는 앱 것이다.
        assert_eq!(wheel_target(WheelCtx { alt_screen: true, ..both }), WheelTo::Nothing);
    }

    /// 사용자가 켠 pane: 오버레이가 닫혀 있으면 위로 굴릴 때 먼저 연다(Ctrl+T).
    /// 열린 뒤에는 페이지 키, Shift면 언제나 우리 스크롤백.
    #[test]
    fn user_forced_keys_open_overlay_then_page() {
        let c = WheelCtx { force_keys: true, up: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::OpenTui);
        assert_eq!(wheel_target(WheelCtx { overlay: true, ..c }), WheelTo::PageKeys);
        assert_eq!(wheel_target(WheelCtx { overlay: true, up: false, ..c }), WheelTo::PageKeys);
        // 오버레이도 없는데 아래로 — 볼 과거가 없으니 아무것도 보내지 않는다.
        assert_eq!(wheel_target(WheelCtx { up: false, ..c }), WheelTo::Nothing);
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::Scrollback);
    }

    /// 오버레이 판정은 화면 하단 안내줄로 한다(키 추적은 codex Esc=편집 때문에 폐기).
    #[test]
    fn overlay_detected_from_footer_text() {
        assert!(overlay_marker("\u{2191}/\u{2193} to scroll   pgup/pgdn to page\n q to quit   esc to edit prev"));
        assert!(overlay_marker("q to quit   esc/\u{2190} to edit prev   \u{2192} to edit next"));
        assert!(!overlay_marker("\u{203a} Implement feature\n  gpt-5.6-sol default \u{b7} ~"));
        assert!(!overlay_marker(""));
    }

    /// codex 자동 감지 — 경로·확장자·인자가 붙어도 첫 토큰 basename으로 잡는다.
    #[test]
    fn detects_codex_command() {
        assert!(is_tui_history_app("codex"));
        assert!(is_tui_history_app("codex resume --last"));
        assert!(is_tui_history_app(r"C:\Users\u\AppData\Roaming\npm\codex.cmd --model x"));
        assert!(is_tui_history_app("/usr/local/bin/codex"));
        assert!(!is_tui_history_app("claude --continue"));
        assert!(!is_tui_history_app("cargo build"));
        // 이름에 codex가 '포함'된 다른 명령에 속지 않는다.
        assert!(!is_tui_history_app("codex-helper run"));
        assert!(!is_tui_history_app(""));
    }

    /// OpenTui는 Ctrl+T 한 번만 보낸다(스크롤 키를 겹쳐 보내면 열리자마자 튄다).
    #[test]
    fn open_tui_sends_only_the_toggle() {
        assert_eq!(wheel_bytes(WheelTo::OpenTui, 1.0, false), Some(vec![0x14]));
    }

    /// 마우스 보고를 켠 앱에는 **대체 화면에서만** 양보한다 — 주 화면엔 스크롤백이 있다.
    #[test]
    fn mouse_reporting_app_is_not_doubled() {
        let c = WheelCtx { mouse_on: true, ..Default::default() };
        assert_eq!(wheel_target(c), WheelTo::Scrollback); // 주 화면은 스크롤백이 우선.
        assert_eq!(wheel_target(WheelCtx { shift: true, ..c }), WheelTo::Nothing);
        assert_eq!(wheel_target(WheelCtx { alt_screen: true, ..c }), WheelTo::Nothing);
        // 주 화면에서 1007까지 켠 앱 — 예전에는 여기서 휠이 죽었다(Claude Code 사례).
        assert_eq!(wheel_target(WheelCtx { alt_scroll: true, ..c }), WheelTo::Scrollback);
    }

    /// 주 화면에서 Shift+휠은 언제나 우리 스크롤백이다(앱이 무엇을 켰든).
    #[test]
    fn shift_wheel_always_shows_our_scrollback_on_the_primary_screen() {
        for c in [
            WheelCtx { shift: true, ..Default::default() },
            WheelCtx { shift: true, alt_scroll: true, ..Default::default() },
            WheelCtx { shift: true, force_keys: true, ..Default::default() },
        ] {
            assert_eq!(wheel_target(c), WheelTo::Scrollback);
        }
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
