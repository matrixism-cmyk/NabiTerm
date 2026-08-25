//! 키워드 하이라이트 규칙 파싱 — `단어`, `단어=#RRGGBB`, `re:정규식`(로그 모니터링).
//!
//! ## 왜 색을 뒤에서 찾는가
//!
//! 예전에는 **첫** `=`에서 잘랐다. 단어에는 `=`가 잘 없으니 그동안 괜찮았지만, 정규식이
//! 들어오면 `re:a=b` 같은 패턴이 흔하고 그러면 `b`를 색으로 오해한다. 그래서 **뒤에서**
//! 찾고, 그 뒤가 진짜 색일 때만 색으로 본다. 옛 규칙은 그대로 동작한다.

use nabi_types::Rgba;

/// 한 규칙.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HlRule {
    /// 찾을 글자(정규식이면 정규식 원문).
    pub pat: String,
    pub color: Rgba,
    pub regex: bool,
}

/// 정규식 규칙임을 나타내는 앞머리.
const RE: &str = "re:";

/// 규칙 한 줄을 판다. 빈 패턴이면 None.
pub(crate) fn parse_highlight_rule(s: &str, default: Rgba) -> Option<HlRule> {
    let (body, color) = split_color(s, default);
    let body = body.trim();
    match body.strip_prefix(RE) {
        Some(re) => {
            let re = re.trim();
            (!re.is_empty()).then(|| HlRule { pat: re.to_string(), color, regex: true })
        }
        None => (!body.is_empty()).then(|| HlRule { pat: body.to_string(), color, regex: false }),
    }
}

/// 뒤쪽 `=#RRGGBB`만 색으로 떼어 낸다. 색이 아니면 통째로 패턴이다.
fn split_color(s: &str, default: Rgba) -> (&str, Rgba) {
    if let Some((body, col)) = s.rsplit_once('=') {
        if let Some(c) = Rgba::from_hex(col.trim()) {
            return (body, c);
        }
    }
    (s, default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> Rgba {
        Rgba::rgb(1, 2, 3)
    }

    #[test]
    fn a_plain_word_keeps_working() {
        let r = parse_highlight_rule("ERROR", def()).unwrap();
        assert_eq!((r.pat.as_str(), r.color, r.regex), ("ERROR", def(), false));
    }

    #[test]
    fn a_colour_can_be_given() {
        let r = parse_highlight_rule("WARN=#ff8800", def()).unwrap();
        assert_eq!(r.pat, "WARN");
        assert_eq!(r.color, Rgba::rgb(0xff, 0x88, 0x00));
    }

    /// 색이 아닌 뒤꼬리는 색이 아니다 — 통째로 패턴이다.
    #[test]
    fn a_non_colour_tail_stays_part_of_the_pattern() {
        let r = parse_highlight_rule("X=zzz", def()).unwrap();
        assert_eq!(r.pat, "X=zzz", "색이 아닌 것을 색으로 보고 패턴을 잘랐다");
        assert_eq!(r.color, def());
    }

    #[test]
    fn a_regex_rule_is_recognised() {
        let r = parse_highlight_rule("re:ERROR|FATAL", def()).unwrap();
        assert_eq!((r.pat.as_str(), r.regex), ("ERROR|FATAL", true));
    }

    /// **정규식 안의 `=`를 색으로 오해하면 안 된다** — 뒤에서 찾는 이유가 이것이다.
    #[test]
    fn an_equals_inside_a_regex_is_not_a_colour() {
        let r = parse_highlight_rule("re:a=b", def()).unwrap();
        assert_eq!(r.pat, "a=b");
        assert_eq!(r.color, def());
    }

    #[test]
    fn a_regex_can_still_take_a_colour() {
        let r = parse_highlight_rule(r"re:\d{3}=#00ff00", def()).unwrap();
        assert_eq!(r.pat, r"\d{3}");
        assert_eq!(r.color, Rgba::rgb(0, 0xff, 0));
        assert!(r.regex);
    }

    #[test]
    fn empty_rules_are_dropped() {
        assert!(parse_highlight_rule("", def()).is_none());
        assert!(parse_highlight_rule("   ", def()).is_none());
        assert!(parse_highlight_rule("re:", def()).is_none());
        assert!(parse_highlight_rule("re:  =#ff0000", def()).is_none());
    }
}
