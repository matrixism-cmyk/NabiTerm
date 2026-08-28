//! **스마트케이스** — 대문자를 쓰면 대소문자를 구분한다. 규칙은 여기 하나뿐이다(배치 AD).
//!
//! 같은 규칙이 다섯 군데에 각각 적혀 있었다: 터미널 찾기의 판정과 표시(`nabi-app/find.rs`),
//! 파일 내용 찾기(`findfiles.rs`), 원격 내용 찾기(`sftpgrep.rs`), 그리고 강조
//! (`findhl.rs`). 다섯이 지금은 같은 답을 내지만, **한 곳만 고쳐지는 날**이 온다.
//!
//! 이 배치에서 이미 그 일을 겪었다. 원격 이름 찾기가 두 갈래였고 한쪽만 글로브를 알아서,
//! 같은 질의에 한쪽은 파일을 찾고 다른 쪽은 언제나 빈 결과를 냈다.
//!
//! ## 왜 "대문자가 있으면 구분"인가
//!
//! 사람이 `readme` 라고 칠 때는 대개 아무거나 찾겠다는 뜻이고, `README` 라고 칠 때는 그
//! 이름을 정확히 아는 것이다. 규칙을 따로 켜고 끄게 하지 않아도 의도가 드러난다.

/// 이 질의는 대소문자를 **구분**해야 하는가.
///
/// 대문자가 하나라도 있으면 구분한다. 없으면 무시한다.
pub fn sensitive(query: &str) -> bool {
    query.chars().any(char::is_uppercase)
}

/// 이 질의는 대소문자를 **무시**해야 하는가([`sensitive`]의 반대).
///
/// 두 이름을 다 두는 이유: 부르는 쪽이 `!sensitive(q)` 를 적으면 부정이 하나 늘고, 부정이
/// 늘면 읽는 사람이 틀린다. 쓰는 쪽 말로 부를 수 있게 둔다.
pub fn insensitive(query: &str) -> bool {
    !sensitive(query)
}

/// 표시용 딱지 — 구분하면 `Aa`, 무시하면 `aa`.
///
/// 화면에 무엇이 뜨는지도 규칙과 함께 둔다. 판정과 표시가 갈라지면 화면이 거짓말을 한다.
pub fn label(query: &str) -> &'static str {
    if sensitive(query) {
        "Aa"
    } else {
        "aa"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lowercase_ignores_case() {
        assert!(insensitive("readme"));
        assert!(!sensitive("readme"));
    }

    #[test]
    fn one_uppercase_is_enough_to_be_strict() {
        assert!(sensitive("Readme"));
        assert!(sensitive("readmE"));
    }

    #[test]
    fn digits_and_symbols_do_not_make_it_strict() {
        // 대문자가 아닌 것으로 구분이 켜지면 사용자는 이유를 알 수 없다.
        assert!(insensitive("port-22_x.conf"));
    }

    #[test]
    fn an_empty_query_is_not_strict() {
        assert!(insensitive(""));
    }

    #[test]
    fn the_label_follows_the_same_rule() {
        // 판정과 표시가 갈라지면 화면이 거짓말을 한다.
        assert_eq!(label("readme"), "aa");
        assert_eq!(label("README"), "Aa");
    }

    #[test]
    fn hangul_has_no_case_so_it_is_never_strict() {
        assert!(insensitive("설정파일"));
    }
}
