//! **터미널과 편집기가 같은 폭을 세는가** — 약속을 실제로 시험한다.
//!
//! `nabi-types/textcol.rs` 는 머리말에 이렇게 적어 두었다:
//!
//! > 같은 글자가 터미널에서는 두 칸, 편집기에서는 한 칸으로 계산되면 커서가 어긋난다.
//! > 터미널 그리드(alacritty_terminal)가 unicode-width 기준이므로 여기서도 같은 기준을 쓴다.
//!
//! 그런데 **그 약속을 아무도 시험하지 않았다.** 한쪽은 alacritty 가 세고 다른 쪽은 우리가
//! 세는데, 둘이 같은지 확인한 적이 없다. 크레이트 판이 올라가거나 alacritty 가 규칙을
//! 바꾸면 조용히 갈라진다 — 그리고 그 결과는 "커서가 한 칸씩 밀린다"로만 드러난다.
//!
//! 여기서 하는 일은 하나다. **글자를 진짜 터미널에 흘려 넣고 커서가 몇 칸 갔는지 보고**,
//! 우리 계산과 맞춰 본다. 셈이 아니라 실측이다.

use crate::grid::TermModel;
use nabi_types::GridSize;

/// 그 글을 빈 터미널에 흘려 넣었을 때 커서가 간 열(= 실제로 쓴 칸 수).
fn cols_in_terminal(s: &str) -> usize {
    // 넉넉히 넓게 잡는다 — 줄바꿈이 일어나면 열 수가 아니라 나머지를 재게 된다.
    let mut m = TermModel::new(GridSize::new(200, 5), 10);
    m.process(s.as_bytes());
    m.cursor().col as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 우리 셈과 터미널의 실측이 같아야 한다 — 이것이 textcol.rs 가 약속한 것이다.
    fn agrees(s: &str) {
        let ours = nabi_types::str_cols(s, 4);
        let real = cols_in_terminal(s);
        assert_eq!(ours, real, "{s:?} — 우리 셈 {ours}, 터미널 실측 {real}");
    }

    #[test]
    fn 아스키와_한글은_맞는다() {
        agrees("abc");
        agrees("한글");
        agrees("a한b글c");
    }

    /// 결합 문자는 앞 글자에 붙으므로 칸을 더 쓰지 않는다.
    #[test]
    fn 결합_문자는_칸을_더_쓰지_않는다() {
        agrees("e\u{0301}"); // e + 악센트
        agrees("가\u{0301}");
    }

    /// **여기서 갈릴 수 있다.** 이모지·변형 선택자·ZWJ 는 터미널마다 다르게 센다.
    ///
    /// 갈리면 이 시험이 빨개진다. 그때 고칠 곳은 `textcol.rs` 다 — 터미널 쪽이 기준이고,
    /// 편집기가 그것을 따라간다(커서가 어긋나는 쪽이 편집기이기 때문).
    #[test]
    fn 이모지와_변형선택자도_맞는다() {
        agrees("\u{1f44d}"); // 👍
        agrees("\u{2600}\u{fe0f}"); // ☀ + VS16
        agrees("\u{2600}\u{fe0e}"); // ☀ + VS15
        agrees("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}"); // 👨‍👩‍👧 (ZWJ)
        agrees("\u{1f44d}\u{1f3fd}"); // 👍 + 피부색
    }

    /// 탭은 폭이 앞선 열에 달렸다 — 터미널의 탭 스톱과 같은 자리로 가야 한다.
    #[test]
    fn 탭도_같은_자리로_간다() {
        // 터미널 기본 탭 스톱은 8이다. 우리 기본값 4와 다르므로 8을 준다.
        for s in ["\tx", "a\tx", "abcdefg\tx", "abcdefgh\tx"] {
            let ours = nabi_types::str_cols(s, 8);
            let real = cols_in_terminal(s);
            assert_eq!(ours, real, "{s:?} — 우리 셈 {ours}, 터미널 실측 {real}");
        }
    }
}
