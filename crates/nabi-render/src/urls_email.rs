//! 스킴 없는 이메일 주소(user@host.tld) 인식 — row_urls에서 mailto: 링크로 밑줄·클릭.

/// 위치 i에서 이메일이 시작하면 끝(배타) 인덱스를 돌려준다. 로컬부=[A-Za-z0-9._%+-],
/// 도메인=[A-Za-z0-9.-] + 점 포함 + 끝 라벨(TLD)이 2+ 알파. 끝 마침표는 제외.
pub(crate) fn email_at(chars: &[char], i: usize) -> Option<usize> {
    let boundary = i == 0
        || chars[i - 1].is_whitespace()
        || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '<' | '=');
    if !boundary || !chars.get(i).is_some_and(char::is_ascii_alphanumeric) {
        return None;
    }
    let n = chars.len();
    let mut j = i;
    while j < n && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '.' | '_' | '%' | '+' | '-')) {
        j += 1;
    }
    if j == i || chars.get(j) != Some(&'@') {
        return None;
    }
    let at = j;
    j += 1;
    while j < n && (chars[j].is_ascii_alphanumeric() || matches!(chars[j], '.' | '-')) {
        j += 1;
    }
    let domain: String = chars[at + 1..j].iter().collect();
    let domain = domain.trim_end_matches('.');
    let last = domain.rsplit('.').next().unwrap_or("");
    if !domain.contains('.') || last.len() < 2 || !last.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(at + 1 + domain.len())
}

#[cfg(test)]
mod tests {
    use super::email_at;

    fn end(s: &str) -> Option<usize> {
        let c: Vec<char> = s.chars().collect();
        email_at(&c, 0)
    }

    #[test]
    fn detects_and_rejects() {
        assert_eq!(end("me@x.com"), Some(8)); // 전체.
        assert_eq!(end("a.b+t@sub.example.co"), Some(20));
        assert_eq!(end("me@x.com."), Some(8)); // 끝 마침표 제외.
        assert_eq!(end("user@host"), None); // 도메인 점 없음.
        assert_eq!(end("3@4.5"), None); // TLD 숫자.
        assert_eq!(end("@x.com"), None); // 로컬부 없음.
    }
}
