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
        // **화면 문구가 먼저**, 없으면 제목으로 판정한다(2026-09-01 순서 교정).
        // 화면 문구는 `"openai codex"`·`"claude code v"` 처럼 그 CLI 밖에서는 안 나오는
        // 말이라 확실하고, 제목은 파일 이름이 섞이기 쉬워 짐작에 가깝다. 확실한 것부터 본다.
        title_kind: kind_from_screen(&text).or_else(|| kind_from_title(model.title())),
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
    if s.contains("antigravity") || s.contains("agy v") {
        return Some("agy");
    }
    if s.contains("aider v") || s.contains("aider chat") {
        return Some("aider");
    }
    None
}

/// 창 제목에서 AI CLI 종류를 판정한다(claude는 제목을 "Claude Code"로 바꾼다).
/// 제목은 OSC 0/2라 SSH를 그대로 통과하므로, 원격 pane 판정의 핵심 단서다.
///
/// **낱말 하나가 통째로 그 이름일 때만** 인정한다. 예전에는 어디에 있든 글자만 맞으면
/// 인정해서 `vim aider.py` 나 `codex.md - vim` 같은 제목에 **엉뚱한 CLI 의 명령 바가 떴다**
/// (2026-09-01 재현). 파일 이름은 제목에 들어가기 마련이라 흔한 일이다.
///
/// 첫머리만 보는 것으로는 부족하다 — `~/proj — codex` 처럼 이름을 뒤에 붙이는 제목이
/// 흔하기 때문이다. 그래서 자리를 따지지 않고 **토막이 정확히 그 이름인지**만 본다.
/// `aider.py` 는 토막이 통째로 `aider` 가 아니므로 걸리지 않는다.
///
/// 엉뚱한 CLI 의 명령이 뜨는 것은 불편이 아니라 **눌렀을 때 사고**다(셸에 그대로 찍힌다).
/// 그래서 애매하면 안 뜨는 쪽을 고른다. 이 판정은 셋째 폴백이기도 하다
/// (실행 명령 → 화면 문구 → 제목).
pub(crate) fn kind_from_title(title: &str) -> Option<&'static str> {
    let t = title.to_ascii_lowercase();
    // 제목에는 경로·구분자가 섞인다 — 공백 말고도 흔한 구분자에서 끊는다.
    let words: Vec<&str> = t.split(|c: char| c.is_whitespace() || "—-–|:()[]".contains(c)).collect();
    ["claude", "codex", "agy", "aider", "gemini"].into_iter().find(|k| words.contains(k))
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
    // 앞의 둘은 **모델 이야기밖에 안 되는** 말이라 어디에 있든 믿는다.
    // 뒤의 둘은 흔한 말이라(`git` 도 `npm` 도 쓴다) **줄 첫머리**에서만 본다.
    const EXACT: [&str; 2] = ["set model to ", "kept model as "];
    const LOOSE: [&str; 2] = ["switched to ", "using "];
    for line in screen.lines().rev() {
        let l = line.trim();
        let low = l.to_ascii_lowercase();
        let found = EXACT
            .iter()
            .find_map(|k| low.find(k).map(|at| at + k.len()))
            .or_else(|| LOOSE.iter().find(|k| low.starts_with(**k)).map(|k| k.len()));
        let Some(at) = found else { continue };
        let v = trim_value(l[at..].trim());
        if looks_like_model(v) {
            return Some(v.to_owned());
        }
    }
    None
}

/// 이 글자가 모델 이름처럼 보이는가 — **화면 판독은 짐작이라 문지기가 필요하다.**
///
/// 2026-09-01에 재현해 보니 예전 규칙은 이런 것들을 모델로 읽었다:
/// `"Using cached credentials"` → `cached credentials`,
/// `"using node v22.3.0"` → `node v22.3.0`,
/// `"Switched to branch 'main'"` → `branch 'main'`.
/// 그 글자가 그대로 명령 바에 모델 이름으로 적혔다(사용자 보고의 한 갈래).
///
/// 그래서 넷을 건다. 하나하나가 위의 오탐 하나씩을 막는다.
///
/// 1. **길이·낱말 수** — 문장이 아니라 이름이어야 한다.
/// 2. **숫자가 있어야 한다** — 모델 이름에는 거의 언제나 판 번호가 붙는다
///    (`Opus 5`·`Haiku 4.5`·`gpt-5-codex`). `cached credentials` 가 여기서 걸린다.
/// 3. **따옴표·빗금이 없어야 한다** — `branch 'main'` 이나 경로가 걸린다.
/// 4. **대문자로 시작하거나 붙임표를 품어야 한다** — 제품 이름은 대문자로 쓰고
///    모델 아이디는 붙임표를 쓴다. 소문자로 시작하는 `node v22.3.0` 이 여기서 걸린다.
fn looks_like_model(v: &str) -> bool {
    let n = v.chars().count();
    let shaped = v.starts_with(|c: char| c.is_ascii_uppercase()) || v.contains('-');
    (1..=24).contains(&n)
        && v.split_whitespace().count() <= 4
        && v.chars().any(|c| c.is_ascii_digit())
        && !v.contains(['\'', '"', '/', '\\'])
        && shaped
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

    /// **모델이 아닌 줄을 모델로 읽으면 안 된다.**
    ///
    /// 아래는 전부 2026-09-01에 실제로 통과하던 것들이다 — 그 글자가 그대로 명령 바에
    /// 모델 이름으로 적혔다. 흔한 도구가 찍는 줄이라 어느 화면에나 있다.
    #[test]
    fn ordinary_lines_are_not_model_names() {
        for s in [
            "Using cached credentials",
            "using node v22.3.0",
            "Switched to branch 'main'",
            "Using default profile",
            "npm warn using --force",
            "Using the following settings",
            "using /usr/bin/env",
        ] {
            assert_eq!(detect_model(s), None, "{s:?} 를 모델로 읽었다");
        }
    }

    /// 진짜 모델 이름은 계속 읽어야 한다 — 문지기가 세면 기능이 죽는다.
    #[test]
    fn real_model_names_still_get_through() {
        for (line, want) in [
            ("Using Fable 5 (from .claude/settings.json)", "Fable 5"),
            ("  Kept model as Haiku 4.5", "Haiku 4.5"),
            ("Set model to Opus 5 (1M context)", "Opus 5"),
            ("using gpt-5-codex", "gpt-5-codex"),
            ("Switched to Gemini 3 Pro", "Gemini 3 Pro"),
        ] {
            assert_eq!(detect_model(line).as_deref(), Some(want), "{line:?}");
        }
    }

    /// 흔한 말(`using`·`switched to`)은 **줄 첫머리**에서만 본다 — 문장 한가운데의
    /// 그 말까지 믿으면 로그 한 줄이 모델 이름을 바꿔 버린다.
    #[test]
    fn loose_words_only_count_at_the_start_of_a_line() {
        assert_eq!(detect_model("build finished using Opus 5"), None);
        // 반면 확실한 말은 어디에 있든 믿는다(들여쓰기·접두가 흔하다).
        assert_eq!(detect_model("[info] Set model to Opus 5").as_deref(), Some("Opus 5"));
    }

    /// **파일 이름이 제목에 들어갔다고 그 CLI 의 명령 바가 뜨면 안 된다.**
    ///
    /// 눌렀을 때 엉뚱한 슬래시 명령이 셸에 찍힌다 — 불편이 아니라 사고다.
    #[test]
    fn a_filename_in_the_title_is_not_a_running_cli() {
        for t in ["vim aider.py", "codex.md - vim", "pagy", "nano claude.json", "agyptian"] {
            assert_eq!(kind_from_title(t), None, "{t:?} 를 실행 중인 CLI 로 봤다");
        }
    }

    /// 진짜 CLI 제목은 그대로 알아본다.
    #[test]
    fn real_cli_titles_are_recognised() {
        assert_eq!(kind_from_title("Claude Code"), Some("claude"));
        assert_eq!(kind_from_title("codex"), Some("codex"));
        assert_eq!(kind_from_title("  agy  "), Some("agy"));
        assert_eq!(kind_from_title("gemini cli"), Some("gemini"));
    }

    /// 이름을 **뒤에** 붙이는 제목도 알아본다 — 셸 프롬프트가 흔히 그렇게 쓴다.
    #[test]
    fn the_name_can_sit_anywhere_as_a_whole_word() {
        assert_eq!(kind_from_title("~/proj \u{2014} codex"), Some("codex"));
        assert_eq!(kind_from_title("kim@srv: ~/work (claude)"), Some("claude"));
        assert_eq!(kind_from_title("agy | main"), Some("agy"));
    }
}
