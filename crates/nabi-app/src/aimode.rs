//! AI pane 화면 판독 — 승인 모드·모델·노력 수준·CLI 종류를 화면 텍스트에서 읽는다.
//!
//! CLI는 이 정보를 프로토콜(OSC 등)로 알려주지 않는다. 상태 줄과 출력이 **유일한 진실원**이라
//! 화면을 읽는다. 셸 통합(OSC 133)이 없는 **SSH pane에서도 동작해야 하므로**(원격 서버에는
//! 통합 스크립트가 없다) 실행 명령 대신 화면·제목으로 판정하는 경로가 필요하다.
//!
//! 스캔은 (pane, 화면 세대) 캐시로 묶어 내용이 바뀐 프레임에만 돈다(aicmdbar).

/// 한 번의 화면 스캔 결과(캐시 항목).
#[derive(Clone, Default)]
pub(crate) struct AiScreen {
    /// 이 결과를 만든 화면 세대.
    pub gen: u64,
    /// 승인/권한 모드 i18n 키.
    pub mode: &'static str,
    /// 화면에서 읽은 현재 모델(예: "Fable 5").
    pub model: Option<String>,
    /// 화면에서 읽은 노력 수준(예: "high").
    pub effort: Option<String>,
    /// 창 제목으로 판정한 CLI 종류(셸 통합이 없는 SSH pane 폴백).
    pub title_kind: Option<&'static str>,
}

/// 화면 전체를 한 번 훑어 모드·모델·노력·종류를 뽑는다(내용 변경 프레임에만 호출).
pub(crate) fn scan(model: &nabi_vt::TermModel, gen: u64) -> AiScreen {
    let rows = model.size().rows() as usize;
    let text = model.visible_text(rows);
    AiScreen {
        gen,
        mode: detect_mode(&text),
        model: detect_model(&text),
        effort: detect_effort(&text),
        // 제목(OSC 0/2)이 1순위, 없으면 화면 문구로 판정한다 — CLI가 제목을 바꾸지 않는
        // 환경(일부 원격 셸·TERM 설정)에서도 SSH pane에서 바가 떠야 한다.
        title_kind: kind_from_title(model.title()).or_else(|| kind_from_screen(&text)),
    }
}

/// 화면에 남은 CLI 고유 문구로 종류를 판정한다(제목이 없을 때의 마지막 폴백).
/// 문구는 실제 화면에서 확인한 것만 쓴다 — 오탐하면 엉뚱한 CLI의 명령 바가 뜬다.
pub(crate) fn kind_from_screen(screen: &str) -> Option<&'static str> {
    let s = screen.to_ascii_lowercase();
    if s.contains("claude code v") || s.contains("shift+tab to cycle") {
        return Some("claude");
    }
    if s.contains("openai codex") || s.contains("codex v") {
        return Some("codex");
    }
    if s.contains("gemini cli") || s.contains("gemini.md") {
        return Some("gemini");
    }
    if s.contains("aider v") || s.contains("aider chat") {
        return Some("aider");
    }
    None
}

/// 창 제목에서 AI CLI 종류를 판정한다(claude는 제목을 "Claude Code"로 바꾼다).
/// 제목은 OSC 0/2라 SSH를 그대로 통과하므로, 원격 pane 판정의 핵심 단서다.
pub(crate) fn kind_from_title(title: &str) -> Option<&'static str> {
    let t = title.to_ascii_lowercase();
    ["claude", "codex", "gemini", "aider"].into_iter().find(|k| t.contains(k))
}

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

/// `"Using Fable 5 (from …)"`, `"Kept model as Fable 5"`, `"Set model to Opus 5 (1M context)"`
/// 같은 줄에서 현재 모델명을 뽑는다(가장 아래=최신).
pub(crate) fn detect_model(screen: &str) -> Option<String> {
    const KEYS: [&str; 4] = ["set model to ", "kept model as ", "switched to ", "using "];
    for line in screen.lines().rev() {
        let l = line.trim();
        let low = l.to_ascii_lowercase();
        for k in KEYS {
            let Some(at) = low.find(k) else { continue };
            let rest = l[at + k.len()..].trim();
            let v = trim_value(rest);
            // "using the following"처럼 모델명이 아닌 문장은 배제(길이·단어 수 제한).
            if !v.is_empty() && v.chars().count() <= 24 && v.split_whitespace().count() <= 4 {
                return Some(v.to_owned());
            }
        }
    }
    None
}

/// `"Set effort level to high (saved as …)"` 같은 줄에서 노력 수준을 뽑는다.
pub(crate) fn detect_effort(screen: &str) -> Option<String> {
    const KEYS: [&str; 3] = ["set effort level to ", "effort level set to ", "effort: "];
    const LEVELS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultracode"];
    for line in screen.lines().rev() {
        let low = line.trim().to_ascii_lowercase();
        for k in KEYS {
            let Some(at) = low.find(k) else { continue };
            let rest = trim_value(&low[at + k.len()..]);
            let first = rest.split_whitespace().next().unwrap_or("");
            if LEVELS.contains(&first) {
                return Some(first.to_owned());
            }
        }
    }
    None
}

/// 값 뒤의 부연(`(…)`·`·`·`—`)을 잘라낸다.
fn trim_value(s: &str) -> &str {
    let mut end = s.len();
    for pat in [" (", " \u{b7}", " \u{2014}", " -", ","] {
        if let Some(i) = s.find(pat) {
            end = end.min(i);
        }
    }
    s[..end].trim().trim_end_matches('.')
}

/// Shift+Tab = CSI Z(backtab). ink·crossterm 등 주요 TUI가 이 시퀀스를 shift+tab으로 읽는다.
pub(crate) const SHIFT_TAB: &[u8] = b"\x1b[Z";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_claude_status_line_modes() {
        assert_eq!(detect_mode("> \n  \u{23f5}\u{23f5} auto mode on (shift+tab to cycle)"), "aimode.auto");
        assert_eq!(detect_mode("\u{23f8} plan mode on (shift+tab to cycle)"), "aimode.plan");
        assert_eq!(detect_mode("\u{23f5}\u{23f5} accept edits on (shift+tab to cycle)"), "aimode.accept");
        assert_eq!(detect_mode("\u{23f5}\u{23f5} bypass permissions on"), "aimode.bypass");
    }

    #[test]
    fn last_line_wins_and_unknown_is_safe() {
        let screen = "plan mode on (shift+tab to cycle)\nsome output\nauto mode on (shift+tab to cycle)";
        assert_eq!(detect_mode(screen), "aimode.auto");
        assert_eq!(detect_mode(""), "aimode.unknown");
    }

    /// 재시작 후에도 "지금 쓰는 모델"이 바에 뜨려면 시작 줄에서 읽어야 한다(사용자 요청).
    #[test]
    fn reads_model_from_startup_and_switch_lines() {
        assert_eq!(
            detect_model("Using Fable 5 (from .claude/settings.json) \u{b7} /model").as_deref(),
            Some("Fable 5")
        );
        assert_eq!(detect_model("  Kept model as Fable 5").as_deref(), Some("Fable 5"));
        assert_eq!(
            detect_model("Set model to Opus 5 (1M context) and saved as your default").as_deref(),
            Some("Opus 5")
        );
        // 아래쪽(최신) 줄이 이긴다.
        assert_eq!(detect_model("Set model to Opus 5\nKept model as Haiku 4.5").as_deref(), Some("Haiku 4.5"));
        assert_eq!(detect_model("nothing here"), None);
    }

    #[test]
    fn reads_effort_level() {
        assert_eq!(
            detect_effort("Set effort level to high (saved as your default for new sessions)").as_deref(),
            Some("high")
        );
        assert_eq!(detect_effort("effort: ultracode").as_deref(), Some("ultracode"));
        assert_eq!(detect_effort("Set effort level to bogus"), None, "알 수 없는 단계는 무시");
    }

    /// 제목이 없어도 화면 문구로 판정된다(SSH·원격 셸 폴백).
    #[test]
    fn screen_kind_is_last_resort() {
        assert_eq!(kind_from_screen("auto mode on (shift+tab to cycle)"), Some("claude"));
        assert_eq!(kind_from_screen("Claude Code v2.1.235"), Some("claude"));
        assert_eq!(kind_from_screen("$ ls -al"), None, "일반 셸 출력은 판정하지 않는다");
    }

    #[test]
    fn title_kind_covers_ssh_panes() {
        assert_eq!(kind_from_title("Claude Code"), Some("claude"));
        assert_eq!(kind_from_title("codex \u{2014} ~/proj"), Some("codex"));
        assert_eq!(kind_from_title("ssh nabi@server"), None);
        assert_eq!(kind_from_title(""), None);
    }
}
