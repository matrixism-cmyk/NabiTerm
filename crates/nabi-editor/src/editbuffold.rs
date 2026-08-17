//! 코드 폴딩 코어(T6-3 잔여) — 소스줄↔시각행 간접층 + 들여쓰기 기반 접기 범위(순수).
//!
//! 폴드는 (시작줄, 끝줄) 집합: 시작줄(헤더)은 보이고 시작+1..=끝 줄이 숨는다.
//! rope 편집기(editbufview)가 이 매핑으로 그리기·히트·캐럿 y를 환산한다.
//! 들여쓰기 기반이라 언어 무관(대부분 에디터의 폴백 방식과 동일 사상).

/// 접힘 상태. 범위는 겹치지 않게 유지된다(겹침 토글=기존 해제).
#[derive(Default, Clone)]
pub struct Folds {
    /// 접힌 (시작줄, 끝줄) 오름차순. 시작줄은 보임, (시작+1..=끝)은 숨김.
    ranges: Vec<(usize, usize)>,
    /// 편집 감지용 — 마지막으로 본 총 줄 수(0=미기록).
    pub last_total: usize,
}

impl Folds {
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    /// 이 줄이 접힌 블록의 헤더면 (시작, 끝)을 돌려준다.
    pub fn header_at(&self, line: usize) -> Option<(usize, usize)> {
        self.ranges.iter().copied().find(|(s, _)| *s == line)
    }

    /// 이 줄이 숨겨져 있는가(어느 접힘의 몸통인가).
    pub fn hidden(&self, line: usize) -> bool {
        self.ranges.iter().any(|(s, e)| line > *s && line <= *e)
    }

    /// 접기/펼치기 토글. 겹치는 기존 폴드는 해제 후 새로 접는다.
    pub fn toggle(&mut self, start: usize, end: usize) {
        if end <= start {
            return;
        }
        let before = self.ranges.len();
        self.ranges.retain(|(s, e)| !(start <= *e && *s <= end));
        if self.ranges.len() == before {
            self.ranges.push((start, end));
            self.ranges.sort_unstable();
        }
    }

    /// line을 포함(헤더 포함)하는 폴드를 펼친다.
    pub fn unfold_containing(&mut self, line: usize) {
        self.ranges.retain(|(s, e)| !(line >= *s && line <= *e));
    }

    /// 총 시각행 수 = 전체 줄 - 숨김 줄.
    pub fn visible_count(&self, total: usize) -> usize {
        let hidden: usize = self.ranges.iter().map(|(s, e)| (*e).min(total.saturating_sub(1)).saturating_sub(*s)).sum();
        total.saturating_sub(hidden)
    }

    /// 시각행 → 소스줄. 접힌 몸통을 건너뛰며 센다.
    pub fn vis_to_src(&self, vis: usize) -> usize {
        let mut src = vis;
        for (s, e) in &self.ranges {
            if *s < src {
                src += e - s; // 이 폴드의 숨김 줄 수만큼 뒤로.
            } else {
                break;
            }
        }
        src
    }

    /// 소스줄 → 시각행. 숨겨진 줄은 그 폴드 헤더의 시각행을 돌려준다.
    pub fn src_to_vis(&self, src: usize) -> usize {
        let mut vis = src;
        for (s, e) in &self.ranges {
            if *e < src {
                vis -= e - s;
            } else if src > *s {
                vis -= src - s; // 몸통 안 — 헤더 위치로.
            } else {
                break;
            }
        }
        vis
    }

    /// 편집 반영: 총 줄 수 변화(delta)를 편집 지점(at) 뒤 폴드에 전이하고,
    /// 편집 지점을 포함하는 폴드는 펼친다(내용이 바뀌었으니 보이게).
    pub fn apply_edit(&mut self, at: usize, new_total: usize) {
        let old = self.last_total;
        self.last_total = new_total;
        if old == 0 || old == new_total {
            return;
        }
        let delta = new_total as i64 - old as i64;
        self.unfold_containing(at);
        // 여러 줄 삭제가 폴드 경계를 걸치면 그 폴드도 해제(밀기만으로는 어긋난다).
        if delta < 0 {
            let cut_end = at + delta.unsigned_abs() as usize;
            self.ranges.retain(|(s, e)| *e < at || *s > cut_end);
        }
        for r in &mut self.ranges {
            if r.0 > at {
                r.0 = (r.0 as i64 + delta).max(0) as usize;
                r.1 = (r.1 as i64 + delta).max(0) as usize;
            }
        }
        self.ranges.retain(|(s, e)| e > s && *e < new_total);
    }
}

/// 들여쓰기 기반 접기 범위: line 뒤로 "더 깊은 들여쓰기" 줄들이 이어지는 구간.
/// `indent(i)`는 i번째 줄의 들여쓰기 폭(빈 줄=None). 최소 1줄은 숨겨져야 Some.
pub fn fold_range_at(line: usize, total: usize, indent: impl Fn(usize) -> Option<usize>) -> Option<(usize, usize)> {
    let base = indent(line)?;
    let mut end = line;
    let mut i = line + 1;
    while i < total {
        match indent(i) {
            None => {} // 빈 줄은 판정 유보(뒤에 깊은 줄이 더 있으면 포함).
            Some(d) if d > base => end = i,
            Some(_) => break,
        }
        i += 1;
    }
    (end > line).then_some((line, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_roundtrip_with_folds() {
        let mut f = Folds::default();
        f.toggle(2, 5); // 3~5줄 숨김.
        f.toggle(8, 9); // 9줄 숨김.
        assert_eq!(f.visible_count(12), 12 - 3 - 1);
        // 시각행: 0,1,2(=헤더),6,7,8(=헤더),10,11
        assert_eq!(f.vis_to_src(2), 2);
        assert_eq!(f.vis_to_src(3), 6);
        assert_eq!(f.vis_to_src(5), 8);
        assert_eq!(f.vis_to_src(6), 10);
        assert_eq!(f.src_to_vis(6), 3);
        assert_eq!(f.src_to_vis(4), 2, "숨김 줄은 헤더 시각행");
        assert_eq!(f.src_to_vis(11), 7);
        assert!(f.hidden(3) && !f.hidden(2) && !f.hidden(6));
        // 왕복: 보이는 줄은 vis→src→vis 불변.
        for v in 0..8 {
            assert_eq!(f.src_to_vis(f.vis_to_src(v)), v, "v={v}");
        }
    }

    #[test]
    fn toggle_and_edit_shift() {
        let mut f = Folds::default();
        f.toggle(2, 5);
        f.toggle(3, 4); // 겹침 → 기존 해제(다시 접지 않음).
        assert!(f.is_empty());
        f.toggle(2, 5);
        f.last_total = 20;
        f.apply_edit(0, 22); // 위에서 2줄 삽입 → 폴드가 밀린다.
        assert_eq!(f.header_at(4), Some((4, 7)));
        f.apply_edit(5, 21); // 폴드 안 편집 → 펼침.
        assert!(f.is_empty());
    }

    #[test]
    fn indent_fold_range() {
        // 0: fn a() {      (indent 0)
        // 1:     x;        (4)
        // 2:                (빈 줄)
        // 3:     y;        (4)
        // 4: }             (0)
        let ind = |i: usize| [Some(0), Some(4), None, Some(4), Some(0)].get(i).copied().flatten();
        assert_eq!(fold_range_at(0, 5, ind), Some((0, 3)), "빈 줄 너머 깊은 줄까지");
        assert_eq!(fold_range_at(1, 5, ind), None, "더 깊은 후속 없음");
        assert_eq!(fold_range_at(4, 5, ind), None);
    }
}
