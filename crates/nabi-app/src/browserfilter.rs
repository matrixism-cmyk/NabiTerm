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

/// 필터가 이름과 맞는지 — 공백으로 나눈 토큰 단위. 대소문자는 무시하고, 빈 필터는 전체 통과.
///
/// - 포함 토큰은 **전부** 맞아야 한다(AND, 순서 무관): `main rs` → `main.rs` ✓
/// - `-`나 `!`로 시작하는 토큰은 **제외**다: `*.rs -test` → `.rs`이면서 `test`가 없는 것만.
///   목록에서 잡음을 걷어낼 때 필요한데(로그·백업·테스트 파일) 그동안 포함만 됐다.
/// - 토큰마다 glob 여부를 따로 본다 — 예전엔 필터 전체에 `*`가 하나라도 있으면 문자열
///   통째로 glob 매칭해서 `main *.rs` 같은 조합이 아무것도 못 찾았다.
pub(crate) fn name_matches(filter: &str, name: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let f = filter.to_lowercase();
    let n = name.to_lowercase();
    for tok in f.split_whitespace() {
        match tok.strip_prefix(['-', '!']) {
            Some(neg) if !neg.is_empty() => {
                if token_matches(neg, &n) {
                    return false; // 제외 토큰에 걸리면 즉시 탈락.
                }
            }
            // "-"나 "!" 한 글자만 친 경우는 아직 입력 중이라 보고 무시한다.
            Some(_) => {}
            None => {
                if !token_matches(tok, &n) {
                    return false;
                }
            }
        }
    }
    true // 포함 토큰이 없고 제외에도 안 걸렸으면 통과.
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
}
