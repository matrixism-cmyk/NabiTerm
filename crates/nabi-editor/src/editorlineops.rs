//! nabiPad 줄 단위 편집 — 커서가 속한 줄을 복제/삭제(표준 에디터 Ctrl+D / 줄 삭제). 순수 함수(테스트).

/// cursor_char(문자 인덱스)가 속한 줄의 바이트 범위 [start, end)를 구한다(end는 개행 직전).
fn line_bytes(text: &str, cursor_char: usize) -> (usize, usize) {
    let cur = text.char_indices().nth(cursor_char).map(|(b, _)| b).unwrap_or(text.len());
    let start = text[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[cur..].find('\n').map(|i| cur + i).unwrap_or(text.len());
    (start, end)
}

/// 커서 줄을 바로 아래에 복제한다. (새 텍스트, 새 커서 char 위치).
pub fn dup_line(text: &str, cursor_char: usize) -> (String, usize) {
    let (s, e) = line_bytes(text, cursor_char);
    let line = text[s..e].to_string();
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    out.push_str(&text[..e]);
    out.push('\n');
    out.push_str(&line);
    out.push_str(&text[e..]);
    (out, cursor_char + line.chars().count() + 1)
}

/// 커서 줄을 삭제한다(줄 + 개행 하나). (새 텍스트, 새 커서 char 위치).
pub fn del_line(text: &str, cursor_char: usize) -> (String, usize) {
    let (s, e) = line_bytes(text, cursor_char);
    let (cut_s, cut_e) = if e < text.len() {
        (s, e + 1) // 끝 개행 포함.
    } else if s > 0 {
        (s - 1, e) // 마지막 줄이면 앞 개행 제거.
    } else {
        (s, e) // 유일한 줄.
    };
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..cut_s]);
    out.push_str(&text[cut_e..]);
    let nc = text[..cut_s].chars().count();
    let total = out.chars().count();
    (out, nc.min(total))
}

/// 커서 줄을 위(up=true)/아래로 한 줄 이동(인접 줄과 교환). (새 텍스트, 새 커서 char).
pub fn move_line(text: &str, cursor_char: usize, up: bool) -> (String, usize) {
    let cur = text.char_indices().nth(cursor_char).map(|(b, _)| b).unwrap_or(text.len());
    let line_start = text[..cur].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = text[line_start..cur].chars().count();
    let idx = text[..line_start].matches('\n').count();
    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    let swap = if up {
        if idx == 0 { return (text.to_string(), cursor_char); }
        idx - 1
    } else {
        if idx + 1 >= lines.len() { return (text.to_string(), cursor_char); }
        idx + 1
    };
    lines.swap(idx, swap);
    let new_start: usize = lines[..swap].iter().map(|l| l.chars().count() + 1).sum();
    let out = lines.join("\n");
    (out, new_start + col.min(lines[swap].chars().count()))
}

#[cfg(test)]
mod tests {
    use super::{del_line, dup_line, move_line};

    #[test]
    fn dup_and_del_lines() {
        assert_eq!(dup_line("ab\ncd\nef", 4).0, "ab\ncd\ncd\nef"); // 'cd' 줄(char 3~4) 복제.
        assert_eq!(del_line("ab\ncd\nef", 4).0, "ab\nef"); // 'cd' 줄 삭제.
        assert_eq!(del_line("ab\ncd", 4).0, "ab"); // 마지막 줄 삭제(앞 개행 제거).
        assert_eq!(del_line("solo", 1).0, ""); // 유일한 줄.
    }

    #[test]
    fn moves_lines() {
        assert_eq!(move_line("a\nb\nc", 2, true).0, "b\na\nc"); // 'b'(char2) 위로.
        assert_eq!(move_line("a\nb\nc", 2, false).0, "a\nc\nb"); // 'b' 아래로.
        assert_eq!(move_line("a\nb", 0, true).0, "a\nb"); // 첫 줄 위로=무변화.
    }
}
