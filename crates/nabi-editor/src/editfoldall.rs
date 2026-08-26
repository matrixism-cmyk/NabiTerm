//! **전체 접기 / 전체 펴기** — 문서를 한 번에 접었다 편다.
//!
//! 접기는 한 자리씩만 됐다(`Folds::toggle` + 우클릭 메뉴). 그런데 접기를 쓰는 가장 흔한
//! 순간은 "처음 보는 파일의 뼈대만 훑고 싶을 때"다. 그때 필요한 것은 **전부 접기**이고,
//! 함수 스무 개를 하나씩 접는 것은 그 목적에 아무 도움이 안 된다.
//!
//! ## 어디까지 접는가
//!
//! **가장 바깥 것만** 접는다. 안쪽까지 전부 접어도 화면에 보이는 결과는 같은데(바깥이
//! 접히면 안쪽은 어차피 숨는다), 나중에 바깥을 펴면 안쪽이 접힌 채로 남아 "펴라고 했는데
//! 안 펴진다"로 보인다. 그래서 겉껍질만 접는다.
//!
//! ## 왜 여기서 도는가
//!
//! `fold_range_at`은 한 줄의 범위를 준다. 전부 접으려면 문서를 훑어야 하고, 훑는 규칙
//! (겹치는 것은 건너뛴다)이 화면 코드에 섞이면 시험할 수 없다. 그래서 순수 함수로 뺀다.

use crate::editbuffold::{fold_range_at, Folds};

/// 접을 수 있는 **가장 바깥** 범위들. `indent(line)`은 그 줄의 들여쓰기 깊이(빈 줄은 None).
pub fn outermost_ranges(total: usize, indent: impl Fn(usize) -> Option<usize>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut line = 0usize;
    while line < total {
        match fold_range_at(line, total, &indent) {
            // 접히는 자리를 찾으면 그 범위 **다음**으로 건너뛴다 — 안쪽은 보지 않는다.
            Some((s, e)) => {
                out.push((s, e));
                line = e + 1;
            }
            None => line += 1,
        }
    }
    out
}

/// 문서 전체를 접는다. 이미 접힌 것은 그대로 둔다(두 번 눌러 펴지면 안 된다).
pub fn fold_all(folds: &mut Folds, total: usize, indent: impl Fn(usize) -> Option<usize>) -> usize {
    let mut n = 0;
    for (s, e) in outermost_ranges(total, indent) {
        if folds.header_at(s).is_none() {
            folds.toggle(s, e);
            n += 1;
        }
    }
    n
}

/// 전부 편다.
pub fn unfold_all(folds: &mut Folds) {
    folds.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 들여쓰기 표를 그대로 쓰는 시험용 도우미(None = 빈 줄).
    fn ind(v: &'static [Option<usize>]) -> impl Fn(usize) -> Option<usize> {
        move |i: usize| v.get(i).copied().flatten()
    }

    #[test]
    fn it_finds_each_top_level_block() {
        // fn a() {   0
        //   x        1
        // fn b() {   0
        //   y        1
        let v = ind(&[Some(0), Some(2), Some(0), Some(2)]);
        assert_eq!(outermost_ranges(4, v), vec![(0, 1), (2, 3)]);
    }

    /// **안쪽은 접지 않는다.** 바깥이 접히면 어차피 숨고, 남겨 두면 나중에 안 펴진 것처럼 보인다.
    #[test]
    fn nested_blocks_are_left_alone() {
        // 0: fn a
        // 1:   if
        // 2:     x
        // 3: fn b
        let v = ind(&[Some(0), Some(2), Some(4), Some(0)]);
        let r = outermost_ranges(4, v);
        assert_eq!(r, vec![(0, 2)], "안쪽 if까지 접었다: {r:?}");
    }

    #[test]
    fn a_flat_document_has_nothing_to_fold() {
        let v = ind(&[Some(0), Some(0), Some(0)]);
        assert!(outermost_ranges(3, v).is_empty());
    }

    #[test]
    fn folding_everything_hides_the_bodies() {
        let mut f = Folds::default();
        let v = ind(&[Some(0), Some(2), Some(0), Some(2)]);
        assert_eq!(fold_all(&mut f, 4, v), 2);
        assert!(f.hidden(1) && f.hidden(3), "본문이 안 숨었다");
        assert!(!f.hidden(0) && !f.hidden(2), "머리줄은 보여야 한다");
    }

    /// **두 번 눌러도 펴지지 않는다.** `toggle`을 그대로 다시 부르면 접은 것이 풀린다.
    #[test]
    fn folding_twice_does_not_unfold() {
        let mut f = Folds::default();
        let v = ind(&[Some(0), Some(2), Some(0), Some(2)]);
        fold_all(&mut f, 4, &v);
        let again = fold_all(&mut f, 4, &v);
        assert_eq!(again, 0, "이미 접힌 것을 다시 건드렸다");
        assert!(f.hidden(1), "두 번째 호출에 펴져 버렸다");
    }

    #[test]
    fn unfolding_everything_clears_it() {
        let mut f = Folds::default();
        let v = ind(&[Some(0), Some(2)]);
        fold_all(&mut f, 2, v);
        unfold_all(&mut f);
        assert!(f.is_empty());
        assert!(!f.hidden(1));
    }

    /// 빈 문서에서 아무 일도 일어나지 않는다.
    #[test]
    fn an_empty_document_is_fine() {
        let mut f = Folds::default();
        assert_eq!(fold_all(&mut f, 0, ind(&[])), 0);
    }
}
