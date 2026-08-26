//! **짝이 되는 글자를 한 곳에서 정한다** — 자동 닫기와 감싸기가 같은 표를 본다.
//!
//! 감싸기(선택을 괄호로 두르기)는 이미 자기 목록을 들고 있었다. 자동 닫기가 생기면서
//! 같은 표가 두 벌이 될 뻔했는데, 그러면 언젠가 한쪽에만 글자가 늘고 **감싸기로는 되는데
//! 자동으로는 안 닫히는** 짝이 생긴다.
//!
//! ## 자동 닫기를 언제 하지 않는가
//!
//! 늘 닫으면 성가시다. 특히 따옴표가 그렇다 — `don't`를 치면 `don''t`가 된다. 그래서
//! **바로 뒤에 글자가 붙어 있으면 닫지 않는다.** 여는 괄호는 뒤에 닫는 괄호나 공백이
//! 올 때만 짝을 채운다.

/// 자동으로 닫아 주는 짝들. 감싸기 메뉴도 이 표를 쓴다.
///
/// `<`/`>`는 **넣지 않았다.** 부등호로 쓰는 일이 훨씬 많아 자동으로 닫으면 방해가 된다.
/// 감싸기 메뉴에는 남아 있다 — 거기서는 사용자가 그 짝을 고르는 것이라 뜻이 분명하다.
pub const AUTO: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`')];

/// 감싸기 메뉴에 보일 짝들(자동 닫기 표 + 부등호).
pub const SURROUND: &[(&str, &str)] =
    &[("(", ")"), ("[", "]"), ("{", "}"), ("<", ">"), ("\"", "\""), ("'", "'"), ("`", "`")];

/// 이 글자를 쳤을 때 자동으로 붙일 닫는 글자. 붙이지 않을 자리면 None.
///
/// `next`는 커서 **바로 뒤**의 글자(줄 끝이면 None).
pub fn closing_for(typed: char, next: Option<char>) -> Option<char> {
    let close = AUTO.iter().find(|(o, _)| *o == typed).map(|(_, c)| *c)?;
    // 뒤에 글자가 붙어 있으면 닫지 않는다 — `don't`가 `don''t`가 되는 것을 막는다.
    match next {
        None => Some(close),
        Some(c) if c.is_whitespace() => Some(close),
        // 닫는 괄호 앞에서는 채워도 방해가 되지 않는다: `foo(|)`에서 `[`를 치면 `foo([|])`.
        Some(c) if AUTO.iter().any(|(_, cl)| *cl == c) => Some(close),
        Some(_) => None,
    }
}

/// 방금 자동으로 채운 닫는 글자를 사용자가 또 쳤나 — 그러면 **덧대지 않고 지나간다**.
///
/// 이것이 없으면 `(`를 치고 `)`를 치는 자연스러운 손버릇이 `())`를 만든다.
pub fn should_step_over(typed: char, next: Option<char>) -> bool {
    next == Some(typed) && AUTO.iter().any(|(_, c)| *c == typed)
}

#[cfg(test)]
mod tests {
    use super::{closing_for, should_step_over, AUTO, SURROUND};

    #[test]
    fn an_opening_bracket_at_the_end_of_a_line_closes() {
        assert_eq!(closing_for('(', None), Some(')'));
        assert_eq!(closing_for('[', Some(' ')), Some(']'));
        assert_eq!(closing_for('{', Some('\t')), Some('}'));
    }

    /// **`don't`가 `don''t`가 되면 안 된다** — 이 시험이 자동 닫기의 성패를 가른다.
    #[test]
    fn a_quote_before_a_letter_does_not_close() {
        assert_eq!(closing_for('\'', Some('t')), None);
        assert_eq!(closing_for('"', Some('a')), None);
    }

    /// 닫는 괄호 앞에서는 채워도 방해가 되지 않는다.
    #[test]
    fn closing_inside_an_existing_pair_still_works() {
        assert_eq!(closing_for('[', Some(')')), Some(']'));
        assert_eq!(closing_for('(', Some('}')), Some(')'));
    }

    #[test]
    fn a_plain_letter_closes_nothing() {
        assert_eq!(closing_for('a', None), None);
        assert_eq!(closing_for(')', None), None, "닫는 글자에 또 짝을 붙였다");
    }

    /// **손버릇으로 닫아도 `())`가 되지 않는다.**
    #[test]
    fn typing_the_closer_yourself_steps_over_it() {
        assert!(should_step_over(')', Some(')')));
        assert!(should_step_over('"', Some('"')));
        assert!(!should_step_over(')', Some('x')));
        assert!(!should_step_over('a', Some('a')), "글자까지 건너뛰었다");
    }

    /// 부등호는 자동으로 닫지 않는다(부등호로 쓰는 일이 훨씬 많다).
    #[test]
    fn angle_brackets_are_not_auto_closed_but_can_be_chosen() {
        assert_eq!(closing_for('<', None), None);
        assert!(SURROUND.iter().any(|(o, _)| *o == "<"), "감싸기에서도 사라졌다");
    }

    /// 두 표가 어긋나지 않아야 한다 — 자동으로 닫는 짝은 감싸기에도 있어야 한다.
    #[test]
    fn every_auto_pair_is_also_offered_for_surrounding() {
        for (o, c) in AUTO {
            let (os, cs) = (o.to_string(), c.to_string());
            assert!(
                SURROUND.iter().any(|(a, b)| *a == os && *b == cs),
                "감싸기 표에 없는 짝: {os}{cs}"
            );
        }
    }
}
