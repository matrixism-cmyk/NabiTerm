//! 편집 버퍼에서 **괄호·따옴표를 자동으로 닫는다**. 어떤 짝을 언제 닫는지는 `pairs`가 정한다.
//!
//! 여기서 하는 일은 셋뿐이다:
//!
//! 1. **선택이 있으면 감싼다.** 고른 글을 지우고 괄호를 치는 것이 아니라 두른다 —
//!    편집기에서 이것이 기대되는 동작이고, 실수로 글을 날리는 일도 막는다.
//! 2. **빈 자리에서는 짝을 채운다.** 커서는 가운데 남는다.
//! 3. **손으로 닫으면 지나간다.** `(`를 치고 `)`를 치는 손버릇이 `())`를 만들지 않게.
//!
//! 여러 커서(박스 선택)에서는 **하지 않는다.** 커서마다 짝을 채우면 되돌리기 한 번에
//! 무엇이 사라지는지 알기 어렵고, 이 기능이 벌어 주는 품보다 놀라움이 크다.

use crate::editbuf::EditBuf;

/// 한 글자 입력을 짝 규칙으로 처리했으면 true(그러면 보통 삽입은 건너뛴다).
pub fn handle_typed(eb: &mut EditBuf, t: &str, readonly: bool) -> bool {
    if readonly || eb.sel.len() > 1 {
        return false;
    }
    let mut it = t.chars();
    let (Some(c), None) = (it.next(), it.next()) else {
        return false; // 한 글자일 때만 — 붙여넣기·조합 글자는 그냥 넣는다.
    };
    if let Some((a, b)) = eb.selection() {
        return surround(eb, c, a, b);
    }
    let next = char_after(eb, eb.cursor());
    if crate::pairs::should_step_over(c, next) {
        eb.set_cursor(eb.cursor() + 1); // 이미 있는 닫는 글자를 지나간다.
        return true;
    }
    match crate::pairs::closing_for(c, next) {
        None => false,
        Some(close) => {
            let at = eb.cursor();
            eb.insert(&format!("{c}{close}"));
            eb.set_cursor(at + 1); // 가운데로.
            true
        }
    }
}

/// 고른 글을 짝으로 두른다. 두른 뒤에도 **선택을 지키는** 것이 중요하다 —
/// 여러 겹으로 두르는 일이 흔한데, 선택이 풀리면 다시 골라야 한다.
fn surround(eb: &mut EditBuf, c: char, a: usize, b: usize) -> bool {
    let Some(close) = crate::pairs::AUTO.iter().find(|(o, _)| *o == c).map(|(_, x)| *x) else {
        return false;
    };
    let text = eb.rope.slice(a..b).to_string();
    eb.insert(&format!("{c}{text}{close}"));
    // 두른 글 안쪽만 다시 고른다(괄호는 뺀다 — 다음에 또 두르면 겹이 는다).
    eb.sel = crate::editsel::Selection::single(a + 1, a + 1 + text.chars().count());
    true
}

/// 커서 바로 뒤 글자(줄 끝·문서 끝이면 None). 줄바꿈은 "글자 없음"과 같게 본다.
fn char_after(eb: &EditBuf, at: usize) -> Option<char> {
    if at >= eb.rope.len_chars() {
        return None;
    }
    match eb.rope.char(at) {
        '\n' | '\r' => None,
        c => Some(c),
    }
}

#[cfg(test)]
mod tests {
    use super::handle_typed;
    use crate::editbuf::EditBuf;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "LF")
    }

    #[test]
    fn an_opening_bracket_fills_in_its_partner() {
        let mut b = buf("");
        assert!(handle_typed(&mut b, "(", false));
        assert_eq!(b.rope.to_string(), "()");
        assert_eq!(b.cursor(), 1, "커서가 가운데 있지 않다");
    }

    /// **`don't`가 `don''t`가 되면 안 된다.**
    #[test]
    fn a_quote_typed_before_a_letter_is_left_alone() {
        let mut b = buf("dont");
        b.set_cursor(3); // don|t
        assert!(!handle_typed(&mut b, "'", false), "짝을 채웠다");
        assert_eq!(b.rope.to_string(), "dont", "이 함수가 글을 건드렸다");
    }

    /// 손으로 닫으면 덧대지 않고 지나간다.
    #[test]
    fn typing_the_closer_walks_past_it() {
        let mut b = buf("()");
        b.set_cursor(1);
        assert!(handle_typed(&mut b, ")", false));
        assert_eq!(b.rope.to_string(), "()", "닫는 글자가 하나 더 붙었다");
        assert_eq!(b.cursor(), 2);
    }

    /// **고른 글은 지워지지 않고 둘러진다** — 이걸 틀리면 사용자의 글이 사라진다.
    #[test]
    fn a_selection_is_wrapped_not_replaced() {
        let mut b = buf("hello");
        b.sel = crate::editsel::Selection::single(0, 5);
        assert!(handle_typed(&mut b, "(", false));
        assert_eq!(b.rope.to_string(), "(hello)");
    }

    /// 두른 뒤에도 안쪽이 골라져 있어야 한다(여러 겹으로 두를 수 있게).
    #[test]
    fn wrapping_twice_nests() {
        let mut b = buf("x");
        b.sel = crate::editsel::Selection::single(0, 1);
        handle_typed(&mut b, "(", false);
        handle_typed(&mut b, "[", false);
        assert_eq!(b.rope.to_string(), "([x])");
    }

    /// 읽기 전용에서는 아무것도 하지 않는다.
    #[test]
    fn read_only_is_untouched() {
        let mut b = buf("");
        assert!(!handle_typed(&mut b, "(", true));
        assert_eq!(b.rope.to_string(), "");
    }

    /// 보통 글자는 이 길로 오지 않는다(원래 삽입 경로가 처리한다).
    #[test]
    fn a_plain_letter_is_not_handled_here() {
        let mut b = buf("");
        assert!(!handle_typed(&mut b, "a", false));
        assert_eq!(b.rope.to_string(), "");
    }

    /// 여러 글자(붙여넣기·조합)는 건드리지 않는다.
    #[test]
    fn multi_character_input_is_left_to_the_normal_path() {
        let mut b = buf("");
        assert!(!handle_typed(&mut b, "((", false));
        assert!(!handle_typed(&mut b, "가", false));
    }
}
