//! nabiPad 들여쓰기 변환 — 탭↔공백. 탭 스톱(폭 N의 배수)을 정확히 반영한다.
//! 전체 문서 대상 순수 함수. 메뉴는 폭 4 래퍼([`tabs_to_spaces4`]/[`spaces_to_tabs4`])를 쓴다.

const W: usize = 4; // 기본 탭 폭(대다수 에디터 기본값).

pub(crate) fn tabs_to_spaces4(t: &str) -> String {
    tabs_to_spaces(t, W)
}
pub(crate) fn spaces_to_tabs4(t: &str) -> String {
    spaces_to_tabs(t, W)
}

/// 비어있지 않은 모든 줄 앞에 공백 2칸 추가(들여쓰기).
pub(crate) fn indent2(text: &str) -> String {
    text.split('\n').map(|l| if l.is_empty() { l.to_string() } else { format!("  {l}") }).collect::<Vec<_>>().join("\n")
}

/// 각 줄의 선두를 한 단계 내어쓴다(맨 앞 탭 1개, 또는 공백 최대 2칸 제거).
pub(crate) fn outdent2(text: &str) -> String {
    text.split('\n')
        .map(|l| {
            if let Some(r) = l.strip_prefix('\t') {
                return r.to_string();
            }
            let (mut s, mut n) = (l, 0);
            while n < 2 && s.starts_with(' ') {
                s = &s[1..];
                n += 1;
            }
            s.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 모든 탭을 다음 탭 스톱까지의 공백으로 확장(줄마다 열 추적, CRLF 보존).
pub(crate) fn tabs_to_spaces(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len());
    for (li, line) in text.split('\n').enumerate() {
        if li > 0 {
            out.push('\n');
        }
        let mut col = 0;
        for c in line.chars() {
            if c == '\t' {
                let n = width - (col % width);
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// 선두 들여쓰기(공백/탭 혼합)를 탭+잔여 공백으로 재구성. 줄 중간 공백은 건드리지 않는다.
pub(crate) fn spaces_to_tabs(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len());
    for (li, line) in text.split('\n').enumerate() {
        if li > 0 {
            out.push('\n');
        }
        let indent_n = line.chars().take_while(|&c| c == ' ' || c == '\t').count();
        let (indent, rest) = split_at_chars(line, indent_n);
        let mut col = 0; // 들여쓰기의 시각적 폭(탭 스톱 반영).
        for c in indent.chars() {
            col += if c == '\t' { width - (col % width) } else { 1 };
        }
        out.extend(std::iter::repeat_n('\t', col / width));
        out.extend(std::iter::repeat_n(' ', col % width));
        out.push_str(rest);
    }
    out
}

/// 앞 `n`개 char에서 문자열을 둘로 나눈다(바이트 아닌 char 기준).
fn split_at_chars(s: &str, n: usize) -> (&str, &str) {
    match s.char_indices().nth(n) {
        Some((i, _)) => s.split_at(i),
        None => (s, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_respects_tabstops() {
        assert_eq!(tabs_to_spaces("\tx", 4), "    x");
        assert_eq!(tabs_to_spaces("a\tb", 4), "a   b"); // 'a'(1) 후 다음 스톱(4)까지 3칸.
        assert_eq!(tabs_to_spaces("ab\tc", 4), "ab  c");
    }

    #[test]
    fn collapse_leading_only() {
        assert_eq!(spaces_to_tabs("    x", 4), "\tx");
        assert_eq!(spaces_to_tabs("      x", 4), "\t  x"); // 6칸=탭1+공백2.
        assert_eq!(spaces_to_tabs("  x  y", 4), "  x  y"); // 줄 중간 공백 보존.
    }

    #[test]
    fn indent_outdent() {
        assert_eq!(indent2("a\n\nb"), "  a\n\n  b"); // 빈 줄은 그대로.
        assert_eq!(outdent2("    a\n\tb\nc"), "  a\nb\nc"); // 공백2·탭1 제거.
    }

    #[test]
    fn idempotent_and_crlf() {
        assert_eq!(spaces_to_tabs("\t  x", 4), "\t  x"); // 혼합 들여쓰기 멱등.
        assert_eq!(tabs_to_spaces("a\tb\r\n\tc", 4), "a   b\r\n    c"); // CRLF 보존.
    }
}
