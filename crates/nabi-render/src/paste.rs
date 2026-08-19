//! 붙여넣기 텍스트 위생 처리 — 붙여넣기 주입(제어문자/escape) 방지.

/// 붙여넣기 텍스트에서 위험한 제어문자를 제거한다(탭/개행/캐리지리턴은 보존).
/// ESC·BEL·백스페이스·C1 등은 터미널 제어 시퀀스 주입에 악용될 수 있어 제거한다.
pub fn sanitize_paste(text: &str) -> String {
    text.chars()
        .filter(|&c| matches!(c, '\t' | '\n' | '\r') || !c.is_control())
        .collect()
}

/// 붙여넣기 전에 사용자 확인이 필요한가 — 설정이 켜져 있고 **줄바꿈이 섞여 있을 때**.
///
/// 줄바꿈이 들어가면 셸이 그 줄을 즉시 실행한다. 그래서 클립보드 출처(Ctrl+Shift+V든
/// 클립보드 히스토리든)와 무관하게 같은 판단을 써야 한다 — 한쪽만 막으면 안전장치가 아니다.
/// 캐리지리턴도 개행으로 본다(정규화 전 텍스트가 들어올 수 있다).
pub fn needs_paste_confirm(warn_enabled: bool, text: &str) -> bool {
    warn_enabled && text.contains(['\n', '\r'])
}

/// 개행 위험(설정 연동) **또는** 유니코드 속임(항상)이면 확인을 받는다.
///
/// 속임 경고는 개행 경고와 별개 스위치다 — 개행 확인을 꺼 둔 사람도 눈에 안 보이는
/// 문자가 섞인 붙여넣기는 봐야 한다(위험의 성격이 다르다).
pub fn needs_confirm_any(warn_newline: bool, warn_unicode: bool, text: &str) -> bool {
    needs_paste_confirm(warn_newline, text)
        || (warn_unicode && !crate::pastedeceive::scan(text).is_empty())
}

/// 붙여넣기 개행을 CR 한 형태로 정규화한다(`\r\n`·`\n`·`\r` → `\r`). 셸은 Enter=CR을 기대하므로
/// 멀티라인 붙여넣기 시 줄마다 정확히 한 번 실행되도록 통일한다.
pub fn normalize_newlines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next(); // CRLF → 한 번만.
                }
                out.push('\r');
            }
            '\n' => out.push('\r'),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{needs_paste_confirm, normalize_newlines, sanitize_paste};

    #[test]
    fn strips_control_chars_keeps_text_and_newlines() {
        assert_eq!(sanitize_paste("ls\x1b[31m -la"), "ls[31m -la"); // ESC 제거.
        assert_eq!(sanitize_paste("a\tb\nc\r"), "a\tb\nc\r"); // 탭/개행/CR 보존.
        assert_eq!(sanitize_paste("x\x07\x08y"), "xy"); // BEL/백스페이스 제거.
        assert_eq!(sanitize_paste("plain text"), "plain text");
    }

    #[test]
    fn normalizes_newlines_to_cr() {
        assert_eq!(normalize_newlines("a\r\nb\nc\rd"), "a\rb\rc\rd");
        assert_eq!(normalize_newlines("no breaks"), "no breaks");
    }

    #[test]
    fn confirm_only_when_enabled_and_multiline() {
        assert!(needs_paste_confirm(true, "a\nb"));
        assert!(needs_paste_confirm(true, "a\r"), "CR도 실행을 부른다");
        assert!(!needs_paste_confirm(true, "one line"));
        assert!(!needs_paste_confirm(false, "a\nb"), "설정이 꺼져 있으면 묻지 않는다");
    }
}

#[cfg(test)]
mod confirm_tests {
    use super::needs_confirm_any;

    /// 개행 경고를 꺼 둬도 유니코드 속임은 확인을 받는다(위험의 성격이 다르다).
    #[test]
    fn unicode_risk_confirms_even_with_newline_warning_off() {
        let zwsp = "cu\u{200b}rl https://example.com/install.sh";
        assert!(needs_confirm_any(false, true, zwsp));
        assert!(!needs_confirm_any(false, false, zwsp)); // 속임 경고까지 끄면 통과.
        assert!(!needs_confirm_any(false, true, "curl https://example.com")); // 깨끗한 한 줄.
        assert!(needs_confirm_any(true, false, "a\nb")); // 개행 경고는 그대로.
    }
}
