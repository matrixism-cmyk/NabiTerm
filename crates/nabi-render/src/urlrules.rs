//! **사용자 정의 링크 규칙** — 로그의 낱말을 클릭할 수 있는 주소로 만든다.
//!
//! 우리는 URL과 파일 경로를 안다. 그런데 로그에서 정작 누르고 싶은 것은 `PROJ-1234`나
//! `#4821` 같은 것이다. 그 낱말이 어느 주소로 가는지는 회사마다 다르므로 사용자가 정한다.
//! MobaXterm·iTerm2가 오래 갖고 있는 기능이다.
//!
//! ## 규칙 문법
//!
//! ```text
//! PROJ-\d+ -> https://jira.example.com/browse/$0
//! #(\d+)   -> https://github.com/o/r/issues/$1
//! ```
//!
//! `$0`은 맞은 전체, `$1`부터는 괄호로 묶은 부분이다. 정규식 관례를 그대로 쓴다 — 새 문법을
//! 배우게 하는 것보다 이미 아는 것을 쓰는 편이 낫다.
//!
//! ## 왜 규칙을 전역에 두는가
//!
//! 링크 탐지는 렌더 경로 깊숙한 순수 함수에서 일어난다(`urls::row_urls_from`). 거기까지
//! 설정을 인자로 끌고 가려면 캐시 키를 포함해 여러 층의 서명을 바꿔야 한다. 규칙은 거의
//! 안 바뀌고 UI 스레드에서만 쓰이므로, 앱이 한 번 넣어 두는 방식을 골랐다
//! (`glyphcache::begin_frame`과 같은 결).

/// 규칙 한 줄을 (정규식, 주소 틀)로 판다. 형식이 아니면 None.
pub fn parse_rule(line: &str) -> Option<(String, String)> {
    let (pat, tmpl) = line.split_once("->")?;
    let (pat, tmpl) = (pat.trim(), tmpl.trim());
    if pat.is_empty() || tmpl.is_empty() {
        return None;
    }
    // 주소 틀에 자리표시자가 하나도 없으면 늘 같은 곳으로 간다 — 실수일 가능성이 높지만
    // 일부러 그렇게 쓰는 경우(대시보드 열기)도 있어 막지는 않는다.
    Some((pat.to_string(), tmpl.to_string()))
}

/// 규칙이 쓸 만한가 — 정규식이 컴파일되는가.
pub fn rule_error(line: &str) -> Option<String> {
    let Some((pat, _)) = parse_rule(line) else {
        return Some("form".into()); // `패턴 -> 주소` 꼴이 아니다.
    };
    regex::Regex::new(&pat).err().map(|e| e.to_string())
}

/// 주소 틀에 맞은 값을 끼운다. `$0`=전체, `$1..$9`=괄호 묶음.
pub fn expand(tmpl: &str, caps: &regex::Captures<'_>) -> String {
    let mut out = String::with_capacity(tmpl.len() + 16);
    let mut it = tmpl.chars().peekable();
    while let Some(c) = it.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match it.peek().and_then(|d| d.to_digit(10)) {
            Some(n) => {
                it.next();
                // 없는 묶음은 빈 값으로 — 여기서 터지면 렌더가 죽는다.
                out.push_str(caps.get(n as usize).map_or("", |m| m.as_str()));
            }
            // `$` 뒤가 숫자가 아니면 그대로 둔다(주소에 `$`가 들어갈 수 있다).
            None => out.push('$'),
        }
    }
    out
}

/// 이 줄에서 규칙에 맞는 자리들 — `(문자 시작, 문자 끝(제외), 주소)`.
///
/// 겹치는 것은 앞선 규칙이 이긴다(사용자가 순서로 우선순위를 정한다).
pub fn matches_in(text: &str, rules: &[(regex::Regex, String)]) -> Vec<(usize, usize, String)> {
    let mut out: Vec<(usize, usize, String)> = Vec::new();
    // 바이트 위치를 문자 위치로 되짚기 위한 표.
    let idx: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
    let char_of = |b: usize| idx.partition_point(|x| *x < b);
    for (re, tmpl) in rules {
        for c in re.captures_iter(text) {
            let Some(m) = c.get(0) else { continue };
            if m.start() == m.end() {
                continue; // 빈 일치는 온 줄을 링크로 만든다.
            }
            let (s, e) = (char_of(m.start()), char_of(m.end()));
            if out.iter().any(|(a, b, _)| s < *b && *a < e) {
                continue; // 이미 잡힌 자리와 겹친다.
            }
            out.push((s, e, expand(tmpl, &c)));
        }
    }
    out.sort_by_key(|(s, _, _)| *s);
    out
}

/// 앱이 넣어 두는 규칙들(컴파일된 것). 바뀔 때마다 세대가 오른다.
pub fn set_rules(lines: &[String]) {
    let compiled: Vec<(regex::Regex, String)> = lines
        .iter()
        .filter_map(|l| parse_rule(l))
        .filter_map(|(p, t)| regex::Regex::new(&p).ok().map(|r| (r, t)))
        .collect();
    RULES.with(|c| *c.borrow_mut() = compiled);
    GEN.with(|g| g.set(g.get() + 1));
}

/// 지금 규칙으로 이 줄을 훑는다. 규칙이 없으면 빈 결과(빠른 반환).
pub fn scan(text: &str) -> Vec<(usize, usize, String)> {
    RULES.with(|c| {
        let r = c.borrow();
        if r.is_empty() {
            return Vec::new();
        }
        matches_in(text, &r)
    })
}

/// 규칙 세대 — 화면 링크 캐시 키에 섞어 규칙이 바뀌면 다시 훑게 한다.
pub fn generation() -> u64 {
    GEN.with(|g| g.get())
}


// clippy 1.96의 `missing_const_for_thread_local`이 이 자리에서 순환한다 — const를 빼면
// "넣어라"라고 하고, 넣으면 다시 "넣을 수 있다"라고 한다. 코드는 const 형태가 맞다
// (둘 다 const 생성자다). 린트 쪽 문제이므로 좁게 예외를 둔다.
#[allow(clippy::missing_const_for_thread_local)]
mod cells {
    thread_local! {
        /// 앱이 넣어 둔 규칙들(컴파일된 것). 페인트·호버는 UI 스레드에서만 일어난다.
        pub(super) static RULES: std::cell::RefCell<Vec<(regex::Regex, String)>> =
            const { std::cell::RefCell::new(Vec::new()) };
        /// 규칙이 바뀔 때마다 오르는 세대(화면 링크 캐시 키에 섞인다).
        pub(super) static GEN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
}

use cells::{GEN, RULES};

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(specs: &[&str]) -> Vec<(regex::Regex, String)> {
        specs
            .iter()
            .filter_map(|s| parse_rule(s))
            .filter_map(|(p, t)| regex::Regex::new(&p).ok().map(|r| (r, t)))
            .collect()
    }

    #[test]
    fn a_rule_splits_into_pattern_and_template() {
        let (p, t) = parse_rule(r"PROJ-\d+ -> https://j/browse/$0").unwrap();
        assert_eq!(p, r"PROJ-\d+");
        assert_eq!(t, "https://j/browse/$0");
    }

    #[test]
    fn malformed_rules_are_rejected() {
        assert!(parse_rule("").is_none());
        assert!(parse_rule("no arrow here").is_none());
        assert!(parse_rule(" -> https://x").is_none(), "빈 패턴을 받았다");
        assert!(parse_rule("x -> ").is_none(), "빈 주소를 받았다");
    }

    /// 잘못된 규칙은 **왜 잘못됐는지** 말할 수 있어야 설정 화면에서 알려 준다.
    #[test]
    fn a_broken_rule_reports_why() {
        assert_eq!(rule_error(r"PROJ-\d+ -> https://x/$0"), None);
        assert_eq!(rule_error("no arrow").as_deref(), Some("form"));
        assert!(rule_error("[unclosed -> https://x").is_some());
    }

    #[test]
    fn the_whole_match_goes_into_dollar_zero() {
        let got = matches_in("see PROJ-1234 now", &rules(&[r"PROJ-\d+ -> https://j/browse/$0"]));
        assert_eq!(got.len(), 1);
        assert_eq!(&got[0].2, "https://j/browse/PROJ-1234");
        assert_eq!((got[0].0, got[0].1), (4, 13));
    }

    /// 괄호로 묶은 부분만 쓰는 것이 더 흔하다(`#1234` → 숫자만).
    #[test]
    fn capture_groups_are_available() {
        let got = matches_in("fixes #4821", &rules(&[r"#(\d+) -> https://g/issues/$1"]));
        assert_eq!(&got[0].2, "https://g/issues/4821");
    }

    /// **없는 묶음을 써도 터지면 안 된다** — 렌더 경로에서 패닉하면 화면이 죽는다.
    #[test]
    fn a_missing_group_becomes_empty_instead_of_panicking() {
        let got = matches_in("x AB", &rules(&["AB -> https://x/$3"]));
        assert_eq!(&got[0].2, "https://x/");
    }

    /// 주소에 진짜 `$`가 들어갈 수 있다.
    #[test]
    fn a_dollar_not_followed_by_a_digit_stays() {
        let got = matches_in("AB", &rules(&["AB -> https://x/?q=$&v=$0"]));
        assert_eq!(&got[0].2, "https://x/?q=$&v=AB");
    }

    /// **겹치면 앞선 규칙이 이긴다** — 두 링크가 같은 글자를 덮으면 클릭이 엉킨다.
    #[test]
    fn overlapping_matches_keep_only_the_first_rule() {
        let r = rules(&[r"AB-\d+ -> https://first/$0", r"\d+ -> https://second/$0"]);
        let got = matches_in("AB-12", &r);
        assert_eq!(got.len(), 1);
        assert!(got[0].2.starts_with("https://first/"), "{:?}", got[0].2);
    }

    /// 빈 일치가 온 줄을 링크로 만들면 안 된다.
    #[test]
    fn an_empty_match_is_ignored() {
        assert!(matches_in("abc", &rules(&["x* -> https://x/$0"])).is_empty());
    }

    /// **한글 뒤에서도 자리가 맞아야 한다** — 바이트와 문자를 헷갈리면 여기서 드러난다.
    #[test]
    fn positions_are_char_indices_not_bytes() {
        let got = matches_in("가나다 PROJ-7", &rules(&[r"PROJ-\d+ -> https://x/$0"]));
        assert_eq!((got[0].0, got[0].1), (4, 10), "한글 뒤 자리가 밀렸다: {got:?}");
    }

    #[test]
    fn several_matches_come_back_in_order() {
        let got = matches_in("A-1 and A-2", &rules(&[r"A-\d -> https://x/$0"]));
        assert_eq!(got.len(), 2);
        assert!(got[0].0 < got[1].0);
    }

    #[test]
    fn no_rules_means_no_matches() {
        assert!(matches_in("PROJ-1", &[]).is_empty());
    }

    /// 규칙이 바뀌면 세대가 올라야 화면 캐시가 다시 훑는다.
    #[test]
    fn changing_the_rules_bumps_the_generation() {
        let before = generation();
        set_rules(&["A -> https://x/$0".to_string()]);
        assert!(generation() > before);
        set_rules(&[]); // 시험 사이 상태를 남기지 않는다.
    }
}
