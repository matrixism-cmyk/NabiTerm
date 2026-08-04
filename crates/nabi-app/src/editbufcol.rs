//! 한 줄의 **표시 문자열**(탭 확장)과 원본↔표시 대응 + grapheme 경계.
//!
//! 페인트와 히트테스트가 각자 열을 계산하면 반드시 어긋난다. 두 쪽 모두 여기서 만든
//! [`DispLine`] 하나를 통해 좌표를 주고받는다. 폭·탭 스톱 기준은 터미널과 공유한다
//! (`nabi_types::textcol`).

use unicode_segmentation::UnicodeSegmentation;

/// 한 줄의 표시 형태와 인덱스 대응.
pub(crate) struct DispLine {
    /// 탭을 공백으로 편 표시 문자열(그대로 그린다).
    pub text: String,
    /// 원본 char i → 표시 char 시작 인덱스(길이 = 원본 char 수 + 1).
    starts: Vec<usize>,
    /// 원본 char i → 표시 **열**(넓은 글자는 2칸). 탭 스톱 계산 기준.
    cols: Vec<usize>,
}

impl DispLine {
    pub(crate) fn new(src: &str, tab: usize) -> Self {
        let n = src.chars().count();
        let mut text = String::with_capacity(src.len());
        let (mut starts, mut cols) = (Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
        let (mut disp, mut col) = (0usize, 0usize);
        for c in src.chars() {
            starts.push(disp);
            cols.push(col);
            if c == '\t' {
                let stop = nabi_types::tab_stop(col, tab);
                text.extend(std::iter::repeat_n(' ', stop - col));
                disp += stop - col;
                col = stop;
            } else {
                text.push(c);
                disp += 1;
                col += nabi_types::char_cols(c);
            }
        }
        starts.push(disp);
        cols.push(col);
        DispLine { text, starts, cols }
    }

    /// 원본 char 인덱스 → 표시 char 인덱스.
    pub(crate) fn to_disp(&self, src: usize) -> usize {
        self.starts[src.min(self.starts.len() - 1)]
    }

    /// 원본 char 인덱스 → 표시 열(넓은 글자 2칸·탭 확장 반영).
    pub(crate) fn col(&self, src: usize) -> usize {
        self.cols[src.min(self.cols.len() - 1)]
    }

    /// 원본 char 수.
    pub(crate) fn chars(&self) -> usize {
        self.starts.len() - 1
    }

    /// 줄 전체가 차지하는 열 수.
    pub(crate) fn width(&self) -> usize {
        self.cols[self.cols.len() - 1]
    }

    /// 표시 char 인덱스 → 가장 가까운 원본 char 인덱스(클릭 지점 → 커서).
    pub(crate) fn to_src(&self, disp: usize) -> usize {
        match self.starts.binary_search(&disp) {
            Ok(i) => i,
            Err(i) => {
                let hi = i.min(self.starts.len() - 1);
                let lo = hi.saturating_sub(1);
                if disp.saturating_sub(self.starts[lo]) <= self.starts[hi] - disp { lo } else { hi }
            }
        }
    }

    /// 표시 열 → 원본 char 인덱스(위/아래 이동에서 열을 유지할 때).
    pub(crate) fn src_at_col(&self, col: usize) -> usize {
        match self.cols.binary_search(&col) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

/// 줄 안의 grapheme 경계(원본 char 인덱스, 0과 끝 포함).
fn bounds(s: &str) -> Vec<usize> {
    let mut byte_of_char: Vec<usize> = s.char_indices().map(|(b, _)| b).collect();
    byte_of_char.push(s.len());
    let mut out: Vec<usize> = s
        .grapheme_indices(true)
        .filter_map(|(b, _)| byte_of_char.binary_search(&b).ok())
        .collect();
    out.push(byte_of_char.len() - 1);
    out
}

/// `ch` 왼쪽 grapheme 경계(없으면 0). 결합 문자·이모지를 쪼개지 않는다.
pub(crate) fn grapheme_left(s: &str, ch: usize) -> usize {
    let b = bounds(s);
    b.iter().rev().find(|&&i| i < ch).copied().unwrap_or(0)
}

/// `ch` 오른쪽 grapheme 경계(없으면 줄 끝).
pub(crate) fn grapheme_right(s: &str, ch: usize) -> usize {
    let b = bounds(s);
    b.iter().find(|&&i| i > ch).copied().unwrap_or_else(|| s.chars().count())
}

/// `ch`를 grapheme 경계로 맞춘다(클릭이 결합 문자 중간에 떨어졌을 때).
pub(crate) fn grapheme_snap(s: &str, ch: usize) -> usize {
    let b = bounds(s);
    b.iter().rev().find(|&&i| i <= ch).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{grapheme_left, grapheme_right, grapheme_snap, DispLine};

    #[test]
    fn tabs_expand_to_next_stop() {
        let d = DispLine::new("ab\tc", 4);
        assert_eq!(d.text, "ab  c", "ab 다음 탭은 4열까지 = 공백 2개");
        assert_eq!(d.to_disp(3), 4, "탭 뒤 'c'는 표시 4번째");
        assert_eq!(d.width(), 5);
    }

    #[test]
    fn wide_chars_count_two_columns() {
        let d = DispLine::new("한a", 4);
        assert_eq!(d.col(1), 2, "한글 뒤는 2열");
        assert_eq!(d.to_disp(1), 1, "표시 문자 수로는 1번째");
        assert_eq!(d.width(), 3);
    }

    #[test]
    fn tab_stop_follows_display_column_not_char_count() {
        // 넓은 글자 뒤의 탭은 '문자 2개'가 아니라 '열 2칸' 기준으로 스톱을 잡아야 한다.
        let d = DispLine::new("한\tx", 4);
        assert_eq!(d.text, "한  x", "2열 → 4열까지 공백 2칸");
    }

    #[test]
    fn click_maps_back_to_nearest_source_char() {
        let d = DispLine::new("ab\tc", 4);
        assert_eq!(d.to_src(0), 0);
        assert_eq!(d.to_src(4), 3, "탭 확장 끝은 'c' 자리");
        assert_eq!(d.to_src(5), 4, "줄 끝");
    }

    #[test]
    fn vertical_move_keeps_display_column() {
        let d = DispLine::new("한글x", 4);
        assert_eq!(d.src_at_col(4), 2, "4열 = 'x' 자리");
        assert_eq!(d.src_at_col(3), 1, "글자 중간 열은 그 글자 시작으로");
    }

    #[test]
    fn caret_moves_by_grapheme_cluster() {
        // e + 결합 악센트: char 2개지만 커서는 통째로 넘어가야 한다.
        let s = "e\u{0301}x";
        assert_eq!(grapheme_right(s, 0), 2, "결합 악센트를 건너뛴다");
        assert_eq!(grapheme_left(s, 2), 0);
        assert_eq!(grapheme_snap(s, 1), 0, "결합 문자 중간 클릭은 경계로");
        assert_eq!(grapheme_right(s, 2), 3);
        assert_eq!(grapheme_left(s, 0), 0, "줄 처음에서 더 못 간다");
    }
}
