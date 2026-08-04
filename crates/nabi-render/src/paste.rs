//! 붙여넣기 텍스트 위생 처리 — 붙여넣기 주입(제어문자/escape) 방지.

/// 붙여넣기 텍스트에서 위험한 제어문자를 제거한다(탭/개행/캐리지리턴은 보존).
/// ESC·BEL·백스페이스·C1 등은 터미널 제어 시퀀스 주입에 악용될 수 있어 제거한다.
pub fn sanitize_paste(text: &str) -> String {
    text.chars()
        .filter(|&c| matches!(c, '\t' | '\n' | '\r') || !c.is_control())
        .collect()
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
    use super::{normalize_newlines, sanitize_paste};

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
}
