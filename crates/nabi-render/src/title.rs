//! 터미널/탭 제목 위생 처리 — OSC 0/2로 받은 제목 문자열을 탭 라벨에 안전하게 쓴다.

/// 제목 문자열을 정리한다: 제어문자 제거, 연속 공백 1칸 축약, 양끝 트림, max 글자로 말줄임(…).
/// max=0이면 길이 제한 없음.
pub fn clean_title(raw: &str, max: usize) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_ws = false;
    for c in raw.chars() {
        if c.is_control() {
            continue;
        }
        if c.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    let s = out.trim_end().to_string();
    if max == 0 || s.chars().count() <= max {
        return s;
    }
    let keep: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", keep.trim_end()) // 말줄임표(앞 공백 정리).
}

/// 문자열을 max 글자로 가운데를 `…`로 줄인다(경로/제목 표시용). 앞·뒤를 보존해 양끝 식별이 쉽다.
pub fn ellipsize_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if max == 0 || n <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "\u{2026}".to_string();
    }
    let head = max / 2;
    let tail = max - head - 1; // '…' 한 글자 몫.
    let chars: Vec<char> = s.chars().collect();
    let h: String = chars[..head].iter().collect();
    let t: String = chars[n - tail..].iter().collect();
    format!("{h}\u{2026}{t}")
}

#[cfg(test)]
mod tests {
    use super::{clean_title, ellipsize_middle};

    #[test]
    fn middle_ellipsis() {
        assert_eq!(ellipsize_middle("abcdefghij", 7), "abc\u{2026}hij"); // head 3 + … + tail 3.
        assert_eq!(ellipsize_middle("short", 10), "short"); // 한계 이하 그대로.
        assert_eq!(ellipsize_middle("abcdef", 5), "ab\u{2026}ef");
    }

    #[test]
    fn sanitizes_and_truncates() {
        assert_eq!(clean_title("  my\t app \x07run  ", 0), "my app run");
        assert_eq!(clean_title("ls\x1b[0m -la", 0), "ls[0m -la"); // 제어(ESC)만 제거.
        assert_eq!(clean_title("abcdefgh", 5), "abcd\u{2026}"); // 말줄임.
        assert_eq!(clean_title("short", 5), "short"); // 한계 이하 그대로.
        assert_eq!(clean_title("한글 제목 테스트", 4), "한글\u{2026}"); // 유니코드 글자 단위(앞 공백 정리).
    }
}
