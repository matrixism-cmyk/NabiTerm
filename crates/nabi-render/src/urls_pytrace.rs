//! Python 트레이스백 `File "경로", line N` 인식 → Ctrl+클릭 시 nabiPad 해당 줄로 연다.
//! 스캔 위치가 여는 `"`일 때, 따옴표 경로(.확장자) 뒤에 `, line <숫자>`가 오면 인정한다(AI 바이브코딩).

/// i가 `"`이고 `"<경로.ext>", line <줄>` 형태면 (끝 인덱스 exclusive, "경로:줄")을 돌려준다.
/// 따옴표 안은 닫는 `"`까지 그대로(Windows `C:\..` 경로의 `:`·`\`도 포함).
pub(crate) fn pytrace_at(chars: &[char], i: usize) -> Option<(usize, String)> {
    if chars.get(i) != Some(&'"') {
        return None;
    }
    let n = chars.len();
    let mut p = i + 1;
    while p < n && chars[p] != '"' {
        p += 1;
    }
    if chars.get(p) != Some(&'"') || p == i + 1 {
        return None; // 닫는 따옴표 없음 / 빈 경로.
    }
    let path: String = chars[i + 1..p].iter().collect();
    if !has_ext(&path) {
        return None;
    }
    // 닫는 따옴표 다음 `, line <숫자>`(콤마 뒤 공백 가변).
    let mut q = p + 1;
    if chars.get(q) != Some(&',') {
        return None;
    }
    q += 1;
    while chars.get(q) == Some(&' ') {
        q += 1;
    }
    if chars.get(q..q + 4) != Some(&['l', 'i', 'n', 'e'][..]) {
        return None;
    }
    q += 4;
    while chars.get(q) == Some(&' ') {
        q += 1;
    }
    let ds = q;
    while chars.get(q).is_some_and(|c| c.is_ascii_digit()) {
        q += 1;
    }
    if q == ds {
        return None;
    }
    let line: String = chars[ds..q].iter().collect();
    Some((q, format!("{path}:{line}")))
}

/// 파일명이 `.확장자`(영문 1자 이상, ≤8자 영숫자)로 끝나는가 — 따옴표 안 임의 텍스트 오탐 방지.
fn has_ext(path: &str) -> bool {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.').is_some_and(|(_, e)| {
        !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()) && e.chars().any(|c| c.is_ascii_alphabetic())
    })
}

#[cfg(test)]
mod tests {
    use super::pytrace_at;

    fn at(s: &str) -> Option<(usize, String)> {
        let c: Vec<char> = s.chars().collect();
        // 여는 따옴표 위치에서 시작.
        let q = s.find('"')?;
        let qi = s[..q].chars().count();
        pytrace_at(&c, qi)
    }

    #[test]
    fn detects_python_traceback() {
        assert_eq!(at("\"script.py\", line 42").unwrap().1, "script.py:42");
        // 흔한 전체 형태(`File ` 접두 + `, in <module>` 후행)도 경로:줄만 추출.
        assert_eq!(at("File \"app/main.py\", line 7, in <module>").unwrap().1, "app/main.py:7");
        // Windows 절대경로(따옴표 안 `:`·`\` 보존).
        assert_eq!(at("\"C:\\proj\\x.py\", line 3").unwrap().1, "C:\\proj\\x.py:3");
    }

    #[test]
    fn rejects_non_traceback() {
        assert!(at("\"hello\", line 1").is_none()); // 확장자 없음.
        assert!(at("\"x.py\" line 1").is_none()); // 콤마 없음.
        assert!(at("\"x.py\", line abc").is_none()); // 줄 번호 아님.
        assert!(at("\"x.py\", col 5").is_none()); // 'line' 키워드 아님.
    }
}
