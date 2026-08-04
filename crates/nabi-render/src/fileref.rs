//! 터미널 출력의 "파일:줄[:열]" 참조 인식(컴파일러 에러 위치 등).
//! Ctrl+클릭 시 nabiPad에서 해당 줄로 연다(라우팅·열기는 nabi-app).
//!
//! 오탐을 줄이려 경로 토큰이 `.<알파벳 포함 확장자>`로 끝나고 그 뒤에 `:<줄>`이 와야만 인정한다.
//! 예: `src/main.rs:42:5`, `main.rs:10`, `/usr/src/x.c:3:1` → 인식. `12:30`·`3.14:5` → 제외.

use crate::urls::is_path_char;

/// 위치 i에서 파일 참조가 시작하면 끝 인덱스(exclusive)를 돌려준다(아니면 None).
pub(crate) fn file_ref_at(chars: &[char], i: usize) -> Option<usize> {
    // 앞이 경계여야 한다(단어 중간 오탐 방지).
    let boundary = i == 0
        || chars[i - 1].is_whitespace()
        || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '=');
    if !boundary {
        return None;
    }
    let n = chars.len();
    // 경로 토큰(공백·구두점·':' 제외 문자 연속).
    let mut p = i;
    while p < n && is_path_char(chars[p]) {
        p += 1;
    }
    if p == i {
        return None;
    }
    // 토큰이 `.확장자`로 끝나야 하고, 확장자는 영숫자(+알파벳 1자 이상)여야 한다(버전·시간 오탐 방지).
    let dot = (i..p).rev().find(|&k| chars[k] == '.')?;
    let ext = &chars[dot + 1..p];
    let ext_ok = !ext.is_empty()
        && ext.len() <= 8
        && ext.iter().all(|c| c.is_ascii_alphanumeric())
        && ext.iter().any(|c| c.is_ascii_alphabetic());
    if !ext_ok {
        return None;
    }
    // `:줄[:열]` 접미가 있어야 파일 참조로 인정.
    let j = line_col_end(chars, p);
    (j != p).then_some(j)
}

/// p에서 줄/열 접미를 소비한 끝 인덱스를 돌려준다(없으면 p 그대로). 두 형식 지원:
/// `:줄[:열]`(gcc/rust/eslint) 또는 `(줄[,열])`(MSVC/tsc). file_ref_at와 절대경로 분기가 공유(SSOT).
pub(crate) fn line_col_end(chars: &[char], p: usize) -> usize {
    if chars.get(p) != Some(&':') || !chars.get(p + 1).is_some_and(|c| c.is_ascii_digit()) {
        return paren_end(chars, p); // `:줄`이 아니면 MSVC `(줄,열)` 형식 시도.
    }
    let n = chars.len();
    let mut j = p + 1;
    while j < n && chars[j].is_ascii_digit() {
        j += 1;
    }
    // 선택적 `:열`.
    if chars.get(j) == Some(&':') && chars.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
        while j < n && chars[j].is_ascii_digit() {
            j += 1;
        }
    }
    j
}

/// p에서 `(줄[,열])`(MSVC/tsc 컴파일러 형식) 접미를 소비한 끝 인덱스. 닫는 괄호가 없으면 p 그대로.
fn paren_end(chars: &[char], p: usize) -> usize {
    if chars.get(p) != Some(&'(') || !chars.get(p + 1).is_some_and(|c| c.is_ascii_digit()) {
        return p;
    }
    let n = chars.len();
    let mut j = p + 1;
    while j < n && chars[j].is_ascii_digit() {
        j += 1;
    }
    if chars.get(j) == Some(&',') && chars.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
        while j < n && chars[j].is_ascii_digit() {
            j += 1;
        }
    }
    if chars.get(j) == Some(&')') {
        j + 1
    } else {
        p
    }
}

/// 파일 참조 문자열을 (경로, 줄, 열)로 분해한다 — 오른쪽 숫자 세그먼트를 최대 2개(줄·열) 떼어낸다.
/// Windows 드라이브 콜론(`C:\...`)은 경로에 남는다. 줄이 없으면 (경로, None, None).
pub fn parse_file_ref(s: &str) -> (String, Option<u32>, Option<u32>) {
    // MSVC/tsc 형식: `path(줄[,열])` — 끝이 `)`면 마지막 `(` 안의 숫자를 줄·열로.
    if let Some(rest) = s.strip_suffix(')') {
        if let Some(open) = rest.rfind('(') {
            let mut it = rest[open + 1..].split(',');
            if let Some(Ok(line)) = it.next().map(|x| x.trim().parse::<u32>()) {
                let col = it.next().and_then(|x| x.trim().parse::<u32>().ok());
                return (rest[..open].to_string(), Some(line), col);
            }
        }
    }
    let parts: Vec<&str> = s.split(':').collect();
    let mut nums: Vec<u32> = Vec::new(); // 오른쪽부터: [열?, 줄] 또는 [줄].
    let mut cut = parts.len();
    while cut > 1 && nums.len() < 2 {
        match parts[cut - 1].parse::<u32>() {
            Ok(v) => {
                nums.push(v);
                cut -= 1;
            }
            Err(_) => break,
        }
    }
    let path = parts[..cut].join(":");
    match nums.as_slice() {
        [line] => (path, Some(*line), None),
        [col, line] => (path, Some(*line), Some(*col)),
        _ => (path, None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::{file_ref_at, parse_file_ref};

    fn end(s: &str) -> Option<usize> {
        let c: Vec<char> = s.chars().collect();
        file_ref_at(&c, 0)
    }
    fn matched(s: &str) -> Option<String> {
        let c: Vec<char> = s.chars().collect();
        file_ref_at(&c, 0).map(|j| c[..j].iter().collect())
    }

    #[test]
    fn detects_relative_and_unix_refs() {
        assert_eq!(matched("src/main.rs:42:5").as_deref(), Some("src/main.rs:42:5"));
        assert_eq!(matched("main.rs:10").as_deref(), Some("main.rs:10"));
        assert_eq!(matched("/usr/src/x.c:3:1").as_deref(), Some("/usr/src/x.c:3:1"));
        assert_eq!(matched("./a-b/c_d.py:7").as_deref(), Some("./a-b/c_d.py:7"));
    }

    #[test]
    fn stops_at_line_and_trailing_text() {
        // 줄/열 뒤 추가 텍스트는 포함하지 않는다.
        let c: Vec<char> = "main.rs:42: error".chars().collect();
        assert_eq!(file_ref_at(&c, 0), Some("main.rs:42".chars().count()));
    }

    #[test]
    fn rejects_non_file_refs() {
        assert_eq!(end("12:30"), None); // 확장자 없음(시간).
        assert_eq!(end("3.14:5"), None); // 확장자가 숫자만(버전/소수).
        assert_eq!(end("main.rs"), None); // 줄 번호 없음.
        assert_eq!(end("main.rs:"), None); // 줄 번호 비어 있음.
        assert_eq!(end("note:see"), None); // '.' 없음.
    }

    #[test]
    fn requires_boundary() {
        let c: Vec<char> = "xsrc/a.rs:1".chars().collect();
        assert_eq!(file_ref_at(&c, 1), None); // i=1은 'src...'지만 앞 'x'가 경계 아님.
    }

    #[test]
    fn line_col_end_consumes_suffix() {
        use super::line_col_end;
        let c: Vec<char> = "x:42:5 rest".chars().collect();
        assert_eq!(line_col_end(&c, 1), "x:42:5".chars().count()); // :줄:열 소비.
        let c2: Vec<char> = "x:7".chars().collect();
        assert_eq!(line_col_end(&c2, 1), 3); // :줄만.
        let c3: Vec<char> = "x more".chars().collect();
        assert_eq!(line_col_end(&c3, 1), 1); // 접미 없음 → 그대로.
    }

    #[test]
    fn parse_splits_path_line_col() {
        assert_eq!(parse_file_ref("src/main.rs:42:5"), ("src/main.rs".into(), Some(42), Some(5)));
        assert_eq!(parse_file_ref("main.rs:10"), ("main.rs".into(), Some(10), None));
        assert_eq!(parse_file_ref("C:\\a\\x.rs:7"), ("C:\\a\\x.rs".into(), Some(7), None)); // 드라이브 콜론 보존.
        assert_eq!(parse_file_ref("plain"), ("plain".into(), None, None)); // 줄 없음.
    }

    #[test]
    fn msvc_paren_form() {
        // MSVC/tsc `파일(줄,열)` 인식 + 분해.
        assert_eq!(matched("main.cpp(42,5)").as_deref(), Some("main.cpp(42,5)"));
        assert_eq!(matched("a.c(7)").as_deref(), Some("a.c(7)"));
        assert_eq!(parse_file_ref("main.cpp(42,5)"), ("main.cpp".into(), Some(42), Some(5)));
        assert_eq!(parse_file_ref("a.c(7)"), ("a.c".into(), Some(7), None));
        assert_eq!(end("a.c(x)"), None); // 괄호 안이 숫자가 아니면 제외.
        assert_eq!(end("a.c(7"), None); // 닫는 괄호 없으면 제외.
    }
}
