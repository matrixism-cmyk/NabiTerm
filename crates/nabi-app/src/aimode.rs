//! AI CLI 승인/권한 모드 감지 — AI 명령 바의 Shift+Tab 순환 버튼용.
//!
//! CLI는 현재 모드를 화면 하단 상태 줄에 쓴다(claude: `⏵⏵ auto mode on (shift+tab to cycle)`,
//! `⏸ plan mode on …`, `⏵⏵ accept edits on …`). 모드를 알려주는 프로토콜(OSC 등)이 없으므로
//! **화면 텍스트가 유일한 진실원**이다 — 하단 몇 줄만 훑어 표시용 라벨을 고른다.

/// 감지된 모드의 i18n 키. 못 찾으면 `aimode.unknown`(버튼은 여전히 순환 가능).
pub(crate) fn detect_mode(screen: &str) -> &'static str {
    let s = screen.to_ascii_lowercase();
    // 상태 줄은 화면 맨 아래에 있다 — 뒤에서부터 훑어 가장 최근 표시를 채택한다.
    for line in s.lines().rev() {
        // 더 구체적인 문구를 먼저(“bypass permissions”가 “permissions”보다 앞).
        if line.contains("bypass permissions") || line.contains("bypassing permissions") {
            return "aimode.bypass";
        }
        if line.contains("plan mode on") {
            return "aimode.plan";
        }
        if line.contains("accept edits on") {
            return "aimode.accept";
        }
        if line.contains("auto mode on") {
            return "aimode.auto";
        }
        if line.contains("normal mode") {
            return "aimode.normal";
        }
    }
    "aimode.unknown"
}

/// Shift+Tab = CSI Z(backtab). ink·crossterm 등 주요 TUI가 이 시퀀스를 shift+tab으로 읽는다.
pub(crate) const SHIFT_TAB: &[u8] = b"\x1b[Z";

#[cfg(test)]
mod tests {
    use super::detect_mode;

    #[test]
    fn detects_claude_status_line_modes() {
        // 실제 claude 화면 하단 문구(2026-08 기준).
        assert_eq!(detect_mode("> \n  \u{23f5}\u{23f5} auto mode on (shift+tab to cycle)"), "aimode.auto");
        assert_eq!(detect_mode("\u{23f8} plan mode on (shift+tab to cycle)"), "aimode.plan");
        assert_eq!(detect_mode("\u{23f5}\u{23f5} accept edits on (shift+tab to cycle)"), "aimode.accept");
        assert_eq!(detect_mode("\u{23f5}\u{23f5} bypass permissions on"), "aimode.bypass");
        assert_eq!(detect_mode("bypassing permissions"), "aimode.bypass");
    }

    #[test]
    fn last_line_wins_and_unknown_is_safe() {
        // 스크롤백에 옛 모드가 남아 있어도 '가장 아래' 표시가 현재 상태다.
        let screen = "plan mode on (shift+tab to cycle)\nsome output\nauto mode on (shift+tab to cycle)";
        assert_eq!(detect_mode(screen), "aimode.auto");
        assert_eq!(detect_mode(""), "aimode.unknown");
        assert_eq!(detect_mode("just a normal prompt line"), "aimode.unknown");
    }
}
