//! 문자 폭·탭 스톱 — 터미널과 편집기가 **같은 수식**을 쓰도록 한 곳에 둔다.
//!
//! 같은 글자가 터미널에서는 두 칸, 편집기에서는 한 칸으로 계산되면 커서가 어긋난다.
//! 터미널 그리드(alacritty_terminal)가 unicode-width 기준이므로 여기서도 같은 기준을 쓴다.

use unicode_width::UnicodeWidthChar;

/// 탭 폭 기본값(칸). 설정이 없거나 0일 때 쓴다.
pub const DEFAULT_TAB: usize = 4;

/// 한 글자가 차지하는 칸 수. 결합 문자·제어 문자는 0, 동아시아 넓은 글자는 2.
///
/// 탭은 앞선 열에 따라 달라지므로 여기서 다루지 않는다 — [`advance`]를 쓸 것.
pub fn char_cols(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// `col`(0-base) 다음 탭 스톱. 이미 스톱 위에 있어도 **다음** 스톱으로 간다(터미널 규칙).
pub fn tab_stop(col: usize, tab: usize) -> usize {
    let t = if tab == 0 { DEFAULT_TAB } else { tab };
    col + t - (col % t)
}

/// 글자 하나를 지난 뒤의 열. 탭은 탭 스톱까지 건너뛴다.
pub fn advance(col: usize, c: char, tab: usize) -> usize {
    if c == '\t' {
        tab_stop(col, tab)
    } else {
        col + char_cols(c)
    }
}

/// 문자열이 차지하는 칸 수(탭 확장 포함, 0열에서 시작한다고 본다).
pub fn str_cols(s: &str, tab: usize) -> usize {
    s.chars().fold(0, |col, c| advance(col, c, tab))
}

#[cfg(test)]
mod tests {
    use super::{advance, char_cols, str_cols, tab_stop};

    #[test]
    fn wide_and_zero_width() {
        assert_eq!(char_cols('a'), 1);
        assert_eq!(char_cols('한'), 2, "한글은 두 칸 — 터미널 그리드와 같아야 한다");
        assert_eq!(char_cols('\u{0301}'), 0, "결합 악센트는 폭 없음");
    }

    #[test]
    fn tab_goes_to_next_stop() {
        assert_eq!(tab_stop(0, 4), 4);
        assert_eq!(tab_stop(1, 4), 4);
        assert_eq!(tab_stop(3, 4), 4);
        assert_eq!(tab_stop(4, 4), 8, "스톱 위에 있어도 다음 스톱으로");
        assert_eq!(tab_stop(0, 0), 4, "탭 폭 0은 기본값으로 대체");
    }

    #[test]
    fn columns_account_for_tabs_and_width() {
        assert_eq!(str_cols("ab\tc", 4), 5, "ab(2) → 탭 스톱 4 → c");
        assert_eq!(str_cols("한글", 4), 4);
        assert_eq!(advance(2, '\t', 4), 4);
        assert_eq!(advance(2, '한', 4), 4);
    }
}
