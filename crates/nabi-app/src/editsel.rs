//! 편집 선택 모델 — 고정단(anchor)/이동단(head) 범위의 정렬·비겹침 집합.
//!
//! 지금은 항상 범위 1개로 동작하지만 자료구조는 **처음부터 여러 개**를 담는다.
//! 단일 커서(`cursor: usize` + `anchor: Option<usize>`)로 만들어 두면 나중에 멀티커서를
//! 넣을 때 모든 편집 명령을 두 번 고쳐야 한다 — CodeMirror·Helix가 공통으로 채택한
//! "정렬된 비겹침 범위 집합 + 주 범위(primary)" 형태를 미리 맞춰 둔다.
//!
//! 불변식(모든 변경 후 [`Selection::normalize`]가 보장):
//! 1. `ranges`는 시작 위치 오름차순.
//! 2. 겹치거나 맞닿은 범위는 병합된다(같은 자리에서 두 번 입력되는 것을 막는다).
//! 3. `primary`는 항상 유효한 인덱스.

/// 편집 지점 하나. `anchor`는 고정단, `head`는 이동단(방향을 보존한다).
/// 둘이 같으면 캐럿, 다르면 `[start,end)` 범위 선택.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Range {
    pub anchor: usize,
    pub head: usize,
}

impl Range {
    /// 빈 범위(캐럿).
    pub(crate) fn caret(at: usize) -> Self {
        Self { anchor: at, head: at }
    }

    /// 선택 없이 캐럿만 있는 상태인지.
    pub(crate) fn is_caret(&self) -> bool {
        self.anchor == self.head
    }

    pub(crate) fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub(crate) fn end(&self) -> usize {
        self.anchor.max(self.head)
    }
}

/// 편집 선택 — 정렬·비겹침 범위 집합과 주 범위 인덱스.
#[derive(Clone, Debug)]
pub(crate) struct Selection {
    ranges: Vec<Range>,
    primary: usize,
}

impl Default for Selection {
    fn default() -> Self {
        Self::caret(0)
    }
}

impl Selection {
    /// 캐럿 하나짜리 선택.
    pub(crate) fn caret(at: usize) -> Self {
        Self { ranges: vec![Range::caret(at)], primary: 0 }
    }

    /// 범위 하나짜리 선택.
    pub(crate) fn single(anchor: usize, head: usize) -> Self {
        Self { ranges: vec![Range { anchor, head }], primary: 0 }
    }

    /// 주 범위(캐럿 표시·스크롤 기준).
    pub(crate) fn primary(&self) -> Range {
        self.ranges[self.primary]
    }

    /// 주 범위를 바꾼다(이후 [`Self::normalize`]로 불변식 회복).
    pub(crate) fn set_primary(&mut self, r: Range) {
        let i = self.primary;
        self.ranges[i] = r;
        self.normalize();
    }

    /// 모든 범위(정렬 순).
    // 아래 네 메서드는 멀티커서 UI가 붙기 전까지 테스트에서만 쓰인다. 자료구조를 미리 맞추는
    // 것이 이 단계의 목적이므로(C2), 지금 지우면 명령들을 나중에 다시 고쳐야 한다.
    #[allow(dead_code)]
    pub(crate) fn ranges(&self) -> &[Range] {
        &self.ranges
    }

    /// 범위 개수.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.ranges.len()
    }

    /// 범위를 추가한다(겹치면 병합).
    #[allow(dead_code)]
    pub(crate) fn push(&mut self, r: Range) {
        self.ranges.push(r);
        self.primary = self.ranges.len() - 1;
        self.normalize();
    }

    /// 주 범위만 남긴다(Esc 등으로 멀티커서 해제).
    #[allow(dead_code)]
    pub(crate) fn collapse_to_primary(&mut self) {
        let p = self.primary();
        self.ranges = vec![p];
        self.primary = 0;
    }

    /// 정렬 + 겹침 병합 + primary 재지정.
    ///
    /// 병합 시 주 범위의 방향(anchor→head)을 보존한다. 방향을 잃으면 Shift+화살표로
    /// 선택을 되돌릴 때 엉뚱한 쪽이 움직인다.
    pub(crate) fn normalize(&mut self) {
        if self.ranges.len() <= 1 {
            self.primary = 0;
            return;
        }
        let keep = self.ranges[self.primary];
        let mut rs = std::mem::take(&mut self.ranges);
        rs.sort_by_key(|r| (r.start(), r.end()));

        // 1) 겹치거나 맞닿은 것을 구간으로 병합한다(방향은 이 단계에서 잠시 잊는다).
        //    각 구간은 (시작, 끝, 그 구간을 대표할 방향)을 갖는다.
        let mut groups: Vec<(usize, usize, bool)> = Vec::with_capacity(rs.len());
        for r in rs {
            let fwd = r.head >= r.anchor;
            match groups.last_mut() {
                Some(g) if r.start() <= g.1 => {
                    g.1 = g.1.max(r.end()); // 시작은 정렬돼 있어 이미 최소값.
                }
                // 겹치지 않으면 새 구간. 방향은 그 구간을 연 범위의 것을 쓴다.
                _ => groups.push((r.start(), r.end(), fwd)),
            }
        }

        // 2) 주 범위를 품은 구간을 찾아 primary로 삼고, **그 구간만** 주 범위의 방향을 물려받는다.
        //    (방향이 중요한 건 사용자가 지금 늘리고 있는 범위뿐이다.)
        let pi = groups
            .iter()
            .position(|g| g.0 <= keep.start() && keep.end() <= g.1)
            .unwrap_or(0);
        groups[pi].2 = keep.head >= keep.anchor;

        self.ranges = groups
            .into_iter()
            .map(|(s, e, fwd)| if fwd { Range { anchor: s, head: e } } else { Range { anchor: e, head: s } })
            .collect();
        self.primary = pi;
    }
}

#[cfg(test)]
mod tests {
    use super::{Range, Selection};

    #[test]
    fn caret_has_single_empty_range() {
        let s = Selection::caret(5);
        assert_eq!(s.len(), 1);
        assert!(s.primary().is_caret());
        assert_eq!(s.primary().head, 5);
    }

    #[test]
    fn range_start_end_ignore_direction() {
        let fwd = Range { anchor: 2, head: 7 };
        let back = Range { anchor: 7, head: 2 };
        assert_eq!((fwd.start(), fwd.end()), (2, 7));
        assert_eq!((back.start(), back.end()), (2, 7), "역방향도 같은 구간");
        assert!(!fwd.is_caret());
    }

    #[test]
    fn normalize_sorts_and_merges_overlaps() {
        let mut s = Selection::single(10, 14);
        s.push(Range { anchor: 0, head: 3 });
        s.push(Range { anchor: 12, head: 20 }); // 10..14 와 겹침 → 병합.
        assert_eq!(s.len(), 2, "겹친 두 범위는 하나로");
        assert_eq!(s.ranges()[0].start(), 0);
        assert_eq!((s.ranges()[1].start(), s.ranges()[1].end()), (10, 20));
    }

    #[test]
    fn touching_carets_collapse() {
        // 같은 자리 캐럿이 둘이면 타자가 두 번 입력된다 → 반드시 하나로 합쳐야 한다.
        let mut s = Selection::caret(4);
        s.push(Range::caret(4));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn push_makes_new_range_primary() {
        // 새로 추가한 커서가 주 커서가 된다(에디터 관행 — 이후 입력이 그쪽 기준).
        let mut s = Selection::caret(0);
        s.push(Range::caret(10));
        assert_eq!(s.primary().head, 10);
    }

    #[test]
    fn merge_preserves_primary_direction() {
        // 왼쪽으로 끌어 선택하다가 앞쪽 범위를 삼켜도 방향이 유지돼야
        // Shift+화살표가 계속 같은 쪽(머리)을 움직인다.
        let mut s = Selection::single(0, 5); // 앞쪽 범위.
        s.push(Range { anchor: 20, head: 10 }); // 역방향 주 범위.
        s.set_primary(Range { anchor: 20, head: 3 }); // 더 끌어 [0,5)와 겹침.
        let p = s.primary();
        assert_eq!(s.len(), 1, "겹쳤으니 하나로");
        assert_eq!((p.start(), p.end()), (0, 20));
        assert!(p.head < p.anchor, "역방향(머리가 왼쪽) 유지");
    }

    #[test]
    fn collapse_keeps_only_primary() {
        let mut s = Selection::caret(0);
        s.push(Range::caret(10));
        s.push(Range::caret(20));
        assert_eq!(s.len(), 3);
        let p = s.primary();
        s.collapse_to_primary();
        assert_eq!(s.len(), 1);
        assert_eq!(s.primary(), p);
    }
}
