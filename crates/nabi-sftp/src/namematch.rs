//! 원격 파일 **이름 맞추기** — 한 규칙, 두 사용처(배치 AD).
//!
//! 원래 이 규칙은 앱 쪽(`sftpfind.rs`)에만 있었고, 도구막대의 재귀 찾기(`recurse::search`)는
//! 따로 `name.contains(needle)` 을 썼다. 같은 질문에 규칙이 둘이면 답도 둘이 된다 —
//! 실제로 `*.conf` 를 치면 찾기 창은 `.conf` 파일들을 찾아 주는데 도구막대는 하나도 못
//! 찾았다. 사용자에게는 둘 다 "서버에서 이름으로 찾기"인데 말이다.
//!
//! 그래서 규칙을 여기 하나로 모았다 — `sftpfind.rs` 도 이것을 부른다.

pub fn matches(name: &str, query: &str) -> bool {
    let (n, q) = (name.to_lowercase(), query.to_lowercase());
    if q.is_empty() {
        return false;
    }
    if q.contains('*') || q.contains('?') {
        glob(&n, &q)
    } else {
        n.contains(&q)
    }
}

/// 아주 작은 글로브 — `*`(0자 이상)와 `?`(정확히 1자)만. 되돌아가며 맞춘다.
fn glob(name: &str, pat: &str) -> bool {
    let (n, p): (Vec<char>, Vec<char>) = (name.chars().collect(), pat.chars().collect());
    // (이름 위치, 패턴 위치)를 되짚기 위한 표식 — `*`를 만나면 여기로 돌아온다.
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while i < n.len() {
        if j < p.len() && (p[j] == '?' || p[j] == n[i]) {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '*' {
            star = j;
            mark = i;
            j += 1;
        } else if star != usize::MAX {
            j = star + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '*' {
        j += 1;
    }
    j == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_word_matches_anywhere_in_the_name() {
        assert!(matches("server.log", "log"));
        assert!(matches("logfile", "log"));
        assert!(!matches("readme", "log"));
    }

    #[test]
    fn case_is_ignored() {
        assert!(matches("README.md", "readme"));
        assert!(matches("readme.md", "README"));
    }

    #[test]
    fn a_star_glob_works() {
        assert!(matches("sshd.conf", "*.conf"));
        assert!(!matches("sshd.confx", "*.conf"), "글로브는 이름 전체에 맞춘다");
    }

    #[test]
    fn a_question_mark_matches_exactly_one_character() {
        assert!(matches("a.log", "?.log"));
        assert!(!matches("ab.log", "?.log"));
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert!(!matches("anything", ""));
    }

    #[test]
    fn a_glob_query_is_not_taken_literally() {
        // 이 시험이 이 모듈이 생긴 이유다. 예전 도구막대 찾기는 `contains("*.conf")` 였고,
        // 이름에 별표가 든 파일은 없으니 **언제나 아무것도 못 찾았다.**
        assert!(!"sshd.conf".contains("*.conf"), "예전 규칙이라면 못 찾는다");
        assert!(matches("sshd.conf", "*.conf"), "지금 규칙은 찾는다");
    }
}
