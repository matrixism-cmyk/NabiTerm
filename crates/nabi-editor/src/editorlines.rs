//! nabiPad 줄 번호 매기기/제거 — 전체 문서 변환(순수 함수). Edit 메뉴 xform 체인용.

/// 각 줄 앞에 우측정렬 줄 번호 + ": "를 붙인다(폭 = 총 줄 수 자릿수).
pub fn add_line_numbers(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let width = lines.len().to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{:>width$}: {l}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 중복 줄 제거(대소문자 무시, 처음 것만 유지, 순서 보존).
pub fn dedupe_ci(text: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    text.split('\n').filter(|l| seen.insert(l.to_lowercase())).collect::<Vec<_>>().join("\n")
}

/// 인접한 중복 줄만 합친다(Unix `uniq` — 연속된 같은 줄을 하나로, 비인접 중복은 보존).
pub fn uniq_adjacent(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for l in text.split('\n') {
        if out.last() != Some(&l) {
            out.push(l);
        }
    }
    out.join("\n")
}

/// 한 번만 나타나는 줄만 남긴다(2회 이상 등장하는 줄은 전부 제거, 순서 보존).
pub fn keep_unique(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut count = std::collections::HashMap::new();
    for l in &lines {
        *count.entry(*l).or_insert(0u32) += 1;
    }
    lines.iter().filter(|l| count[**l] == 1).copied().collect::<Vec<_>>().join("\n")
}

/// 각 줄의 선두 공백/탭을 제거한다(후행은 보존).
pub fn trim_leading(text: &str) -> String {
    text.split('\n').map(|l| l.trim_start_matches([' ', '\t'])).collect::<Vec<_>>().join("\n")
}

/// 각 줄의 양쪽 공백을 제거한다.
pub fn trim_both(text: &str) -> String {
    text.split('\n').map(str::trim).collect::<Vec<_>>().join("\n")
}

/// 문서 앞·뒤의 빈 줄(공백만 포함)을 제거한다(중간 빈 줄은 보존).
pub fn trim_blank_edges(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let start = lines.iter().position(|l| !l.trim().is_empty());
    match start {
        None => String::new(),
        Some(s) => {
            let e = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap_or(s);
            lines[s..=e].join("\n")
        }
    }
}

/// 모든 줄을 공백 하나로 이어 한 줄로(VS Code Join Lines). 각 줄 trim·빈 줄 제외.
pub fn join_lines(text: &str) -> String {
    text.split('\n').map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

/// 각 줄의 문자 순서를 뒤집는다(거울). 개행은 유지.
pub fn reverse_line_chars(text: &str) -> String {
    text.split('\n').map(|l| l.chars().rev().collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// 줄 안의 연속한 공백/탭을 공백 하나로 합친다(개행은 보존). 표·복사 텍스트 정리용.
pub fn collapse_spaces(text: &str) -> String {
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| {
            let mut out = String::with_capacity(line.len());
            let mut prev_ws = false;
            for c in line.chars() {
                if c == ' ' || c == '\t' {
                    if !prev_ws {
                        out.push(' ');
                    }
                    prev_ws = true;
                } else {
                    out.push(c);
                    prev_ws = false;
                }
            }
            out
        })
        .collect();
    lines.join("\n")
}

/// 타이틀(Proper) 케이스: 각 단어의 첫 글자를 대문자, 나머지는 소문자로(단어=영문 연속).
pub fn to_title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_alnum = false;
    for c in text.chars() {
        if c.is_alphabetic() {
            if prev_alnum {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        prev_alnum = c.is_alphanumeric();
    }
    out
}

/// 연속한 빈 줄(공백만 있는 줄 포함)을 하나로 축약한다(remove_empty와 달리 한 줄은 남김).
pub fn squeeze_blanks(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut prev_blank = false;
    for line in text.split('\n') {
        let blank = line.trim().is_empty();
        if blank {
            if prev_blank {
                continue; // 두 번째 이후 연속 빈 줄은 버린다.
            }
            out.push(""); // 남기는 빈 줄은 공백을 지워 정규화.
        } else {
            out.push(line);
        }
        prev_blank = blank;
    }
    out.join("\n")
}

/// 줄 앞의 "번호 + 구분자" 접두(예: `  12: `, `3. `, `5)`, `7\t`)를 제거한다.
pub fn remove_line_numbers(text: &str) -> String {
    text.split('\n').map(strip_num_prefix).collect::<Vec<_>>().join("\n")
}

/// 선두 공백 다음 숫자 런 + 구분자가 있을 때만 그 접두를 떼낸다(아니면 원본 유지).
fn strip_num_prefix(line: &str) -> &str {
    let lead = line.len() - line.trim_start_matches([' ', '\t']).len();
    let t = &line[lead..];
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return line; // 숫자로 시작하지 않으면 들여쓰기 보존.
    }
    match t[digits..].strip_prefix([':', '.', ')', '\t', ' ']) {
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest), // 구분자 뒤 공백 1개도 흡수.
        None => line, // 구분자 없으면 줄 번호로 보지 않음(예: "42abc").
    }
}

/// 줄바꿈을 LF(\n)로 통일한다(CRLF·CR → LF). 크로스플랫폼 개발·git diff 정리용.
pub fn to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// 줄바꿈을 CRLF(\r\n)로 통일한다(먼저 LF로 정규화 후 변환 — 중복 \r 방지). Windows 도구 호환용.
pub fn to_crlf(text: &str) -> String {
    to_lf(text).replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_roundtrip() {
        assert_eq!(add_line_numbers("a\nb"), "1: a\n2: b");
        let many = (1..=10).map(|n| format!("L{n}")).collect::<Vec<_>>().join("\n");
        let numbered = add_line_numbers(&many);
        assert!(numbered.starts_with(" 1: L1\n")); // 폭 2로 정렬.
        assert!(numbered.contains("\n10: L10"));
        assert_eq!(remove_line_numbers(&numbered), many);
    }

    #[test]
    fn remove_various_separators() {
        assert_eq!(remove_line_numbers("12. item\n3) x\n5\tval"), "item\nx\nval");
        assert_eq!(remove_line_numbers("  7: indented"), "indented");
    }

    #[test]
    fn remove_keeps_non_numbered() {
        assert_eq!(remove_line_numbers("hello\n42abc\n  code"), "hello\n42abc\n  code");
    }

    #[test]
    fn dedupe_and_unique() {
        assert_eq!(dedupe_ci("a\nA\nb\nB\na"), "a\nb"); // 대소문자 무시 첫 항목만.
        assert_eq!(keep_unique("a\nb\na\nc"), "b\nc"); // 1회 등장만 남김.
        assert_eq!(uniq_adjacent("a\na\nb\na\na"), "a\nb\na"); // 인접 중복만 합침(비인접 보존).
    }

    #[test]
    fn line_ops() {
        assert_eq!(trim_leading("  a\n\tb"), "a\nb");
        assert_eq!(trim_both("  a  \n\tb\t"), "a\nb");
        assert_eq!(trim_blank_edges("\n \nx\ny\n\n"), "x\ny");
        assert_eq!(join_lines("a\n  b \n\nc"), "a b c");
        assert_eq!(reverse_line_chars("abc\nXY"), "cba\nYX");
    }

    #[test]
    fn collapse_spaces_runs() {
        assert_eq!(collapse_spaces("a   b\t\tc"), "a b c");
        assert_eq!(collapse_spaces("x  y\nz\t w"), "x y\nz w"); // 개행 보존.
    }

    #[test]
    fn title_case_capitalizes_words() {
        assert_eq!(to_title_case("hello world"), "Hello World");
        assert_eq!(to_title_case("FOO bar-baz"), "Foo Bar-Baz");
        assert_eq!(to_title_case("café crème"), "Café Crème"); // 유니코드.
    }

    #[test]
    fn line_ending_convert() {
        assert_eq!(to_lf("a\r\nb\rc\nd"), "a\nb\nc\nd"); // CRLF·CR 모두 LF로.
        assert_eq!(to_crlf("a\nb"), "a\r\nb");
        assert_eq!(to_crlf("a\r\nb"), "a\r\nb"); // 이미 CRLF면 중복 \r 없음.
    }

    #[test]
    fn squeeze_collapses_runs() {
        assert_eq!(squeeze_blanks("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(squeeze_blanks("a\n  \n\t\nb"), "a\n\nb"); // 공백만 있는 줄도 빈 줄.
        assert_eq!(squeeze_blanks("a\nb"), "a\nb"); // 빈 줄 없으면 그대로.
    }
}
