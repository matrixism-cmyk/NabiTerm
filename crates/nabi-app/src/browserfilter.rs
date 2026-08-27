//! 파일 이름 필터 — glob(`*`/`?`)과 포함/제외 토큰 매칭. 로컬 브라우저와 SFTP 목록 공용.
//!
//! 순수 함수만 모아 둔다(입출력 없음) — 규칙이 미묘해서 테스트로 못 박아 두는 편이 낫다.

/// 간단 glob 매칭(*=임의 길이, ?=한 글자). 대소문자는 호출측에서 맞춘다.
fn glob_match(pat: &[char], s: &[char]) -> bool {
    match pat.first() {
        None => s.is_empty(),
        Some('*') => glob_match(&pat[1..], s) || (!s.is_empty() && glob_match(pat, &s[1..])),
        Some('?') => !s.is_empty() && glob_match(&pat[1..], &s[1..]),
        Some(&c) => !s.is_empty() && s[0] == c && glob_match(&pat[1..], &s[1..]),
    }
}

/// 토큰 하나가 이름과 맞는가 — `*`/`?`가 있으면 glob, 없으면 부분일치(둘 다 소문자 가정).
fn token_matches(tok: &str, name: &str) -> bool {
    if tok.contains('*') || tok.contains('?') {
        glob_match(&tok.chars().collect::<Vec<_>>(), &name.chars().collect::<Vec<_>>())
    } else {
        name.contains(tok)
    }
}

/// **한 번만 준비해 두고 항목마다 다시 쓰는** 필터(배치 Z F3).
///
/// `name_matches`는 항목마다 `filter.to_lowercase()`를 새로 만들었다. 필터는 모든 항목에
/// 대해 같은 문자열인데도 그랬다 — 1만 개짜리 폴더면 **프레임마다 1만 번**이다.
/// 목록 그리기를 보이는 줄만으로 줄여 놔도 앞단이 이러면 소용이 없다.
///
/// 규칙 자체는 `name_matches`와 **같은 코드**를 지난다(아래 `matches`가 유일한 구현이고
/// `name_matches`는 그것을 부르는 껍데기다). 두 벌로 두면 언젠가 한쪽만 고쳐진다.
pub(crate) struct NameFilter {
    /// (제외인가, 소문자 토큰). 빈 벡터면 전체 통과.
    toks: Vec<(bool, String)>,
}

impl NameFilter {
    pub(crate) fn new(filter: &str) -> Self {
        let lower = filter.to_lowercase();
        let mut toks = Vec::new();
        for tok in lower.split_whitespace() {
            match tok.strip_prefix(['-', '!']) {
                // "-"나 "!" 한 글자만 친 경우는 아직 입력 중이라 보고 무시한다.
                Some("") => {}
                Some(neg) => toks.push((true, neg.to_string())),
                None => toks.push((false, tok.to_string())),
            }
        }
        Self { toks }
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        if self.toks.is_empty() {
            return true;
        }
        let n = name.to_lowercase();
        for (neg, tok) in &self.toks {
            match (neg, token_matches(tok, &n)) {
                (true, true) => return false,   // 제외 토큰에 걸리면 즉시 탈락.
                (false, false) => return false, // 포함 토큰은 전부 맞아야 한다.
                _ => {}
            }
        }
        true
    }
}

/// 필터가 이름과 맞는지 — 공백으로 나눈 토큰 단위. 대소문자는 무시하고, 빈 필터는 전체 통과.
///
/// - 포함 토큰은 **전부** 맞아야 한다(AND, 순서 무관): `main rs` → `main.rs` ✓
/// - `-`나 `!`로 시작하는 토큰은 **제외**다: `*.rs -test` → `.rs`이면서 `test`가 없는 것만.
///   목록에서 잡음을 걷어낼 때 필요한데(로그·백업·테스트 파일) 그동안 포함만 됐다.
/// - 토큰마다 glob 여부를 따로 본다 — 예전엔 필터 전체에 `*`가 하나라도 있으면 문자열
///   통째로 glob 매칭해서 `main *.rs` 같은 조합이 아무것도 못 찾았다.
pub(crate) fn name_matches(filter: &str, name: &str) -> bool {
    // 규칙은 `NameFilter::matches` 한 곳에만 있다. 여기서 다시 적으면 언젠가 한쪽만 고쳐진다.
    // 항목이 여럿이면 이 껍데기 대신 `NameFilter::new`를 한 번 만들어 쓸 것 — 그래야
    // 필터 소문자 변환이 항목 수만큼 반복되지 않는다.
    NameFilter::new(filter).matches(name)
}

#[cfg(test)]
mod tests {
    use super::name_matches;
    #[test]
    fn name_matches_glob_and_substr() {
                assert!(name_matches("", "anything"));
        assert!(name_matches("rs", "main.rs")); // 부분일치.
        assert!(name_matches("main rs", "main.rs") && !name_matches("main txt", "main.rs")); // 다중 단어 AND.
        assert!(name_matches("*.rs", "main.rs") && !name_matches("*.rs", "main.txt")); // glob.
        assert!(name_matches("te?t", "test"));
        assert!(name_matches("MAIN*", "main.rs")); // 대소문자 무시.
    }
    /// 제외 토큰(-/!)과 토큰별 glob 판정(S4-32).
    #[test]
    fn name_matches_exclusion_tokens() {
                assert!(name_matches("*.rs -test", "main.rs"));
        assert!(!name_matches("*.rs -test", "main_test.rs")); // 제외에 걸림.
        assert!(!name_matches("!log", "server.log")); // ! 도 제외.
        assert!(name_matches("-log", "notes.txt")); // 제외만 있으면 나머지 통과.
        // 토큰마다 glob을 따로 본다 — 예전엔 이 조합이 아무것도 못 찾았다.
        assert!(name_matches("main *.rs", "main.rs"));
        // 아직 입력 중인 "-" 한 글자는 무시(목록이 갑자기 비지 않게).
        assert!(name_matches("-", "anything"));
    }

    /// 준비된 필터와 껍데기가 **같은 답**을 내는가. 규칙을 한 곳에 모았으니 당연해야 하는데,
    /// 그 당연함이 깨지는 순간이 바로 한쪽만 고쳐진 순간이다.
    #[test]
    fn prepared_filter_agrees_with_the_shim() {
        use super::NameFilter;
        let names = ["main.rs", "test_main.rs", "README.md", "a.log", "MAIN.RS", ""];
        let filters = ["", "rs", "main rs", "*.rs", "*.rs -test", "!log", "-", "te?t", "MAIN*"];
        for f in filters {
            let nf = NameFilter::new(f);
            for n in names {
                assert_eq!(nf.matches(n), name_matches(f, n), "필터 {f:?} 이름 {n:?}");
            }
        }
    }

    #[test]
    fn a_prepared_filter_is_built_once_and_reused() {
        use super::NameFilter;
        // 같은 필터로 여러 번 물어도 답이 흔들리지 않는다(상태를 안 들고 있다).
        let nf = NameFilter::new("*.RS -Test");
        assert!(nf.matches("main.rs"));
        assert!(!nf.matches("test_main.rs"));
        assert!(nf.matches("main.rs"), "두 번째 물음도 같아야 한다");
    }
}
