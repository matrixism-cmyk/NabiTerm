//! **고친 자리를 기억한다** — 찾다가 자리를 잃었을 때 돌아갈 길.
//!
//! 문서를 고치다 다른 곳을 찾아보고 나면, 아까 고치던 자리로 돌아가는 데 스크롤이 든다.
//! 되돌리기로 돌아갈 수도 있지만 그건 **고친 것을 무르는** 일이라 뜻이 다르다.
//!
//! ## 왜 하나가 아니라 몇 개인가
//!
//! "마지막 한 자리"만 기억하면, 이미 그 자리에 있을 때 눌러도 아무 일이 없다. 그래서
//! 최근 자리 몇 개를 두고 **누를 때마다 다음 자리로 돈다.** 편집기들이 대개 이렇게 한다.
//!
//! ## 같은 줄은 한 자리로 친다
//!
//! 한 줄을 고치는 동안 글자마다 자리를 남기면 목록이 그 줄로 가득 찬다. 붙어 있는 자리는
//! 최신 것으로 **덮어쓴다**.

/// 기억할 자리 수. 늘리면 도는 데 오래 걸려 오히려 못 찾는다.
const MAX: usize = 8;
/// 이만큼 안쪽이면 같은 자리로 본다(대략 한 줄).
const NEAR: usize = 80;

/// 최근 고친 자리들(최신이 앞).
#[derive(Debug, Default, Clone)]
pub struct EditSpots {
    spots: Vec<usize>,
    /// 지금 몇 번째 자리를 보고 있나(누를 때마다 하나씩 뒤로).
    at: usize,
}

impl EditSpots {
    /// 고친 자리를 남긴다.
    pub fn record(&mut self, pos: usize) {
        // 가까운 자리는 새 자리가 아니다 — 최신 것으로 덮는다.
        self.spots.retain(|p| p.abs_diff(pos) > NEAR);
        self.spots.insert(0, pos);
        self.spots.truncate(MAX);
        self.at = 0; // 새로 고쳤으면 도는 차례도 처음부터.
    }

    /// 글이 바뀌어 자리가 밀렸을 수 있다 — 문서 끝을 넘는 자리는 버린다.
    pub fn clamp(&mut self, len: usize) {
        self.spots.retain(|p| *p <= len);
    }

    /// 다음으로 갈 자리. 기억한 것이 없으면 None.
    ///
    /// `cur`는 지금 커서 — **이미 그 자리에 있으면 건너뛴다**(눌렀는데 안 움직이면 고장으로 읽힌다).
    pub fn next(&mut self, cur: usize) -> Option<usize> {
        if self.spots.is_empty() {
            return None;
        }
        for _ in 0..self.spots.len() {
            let p = self.spots[self.at % self.spots.len()];
            self.at = (self.at + 1) % self.spots.len();
            if p.abs_diff(cur) > NEAR {
                return Some(p);
            }
        }
        None // 기억한 자리가 전부 지금 자리 근처다.
    }

    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::EditSpots;

    #[test]
    fn nothing_recorded_means_nowhere_to_go() {
        let mut s = EditSpots::default();
        assert!(s.is_empty());
        assert_eq!(s.next(0), None);
    }

    #[test]
    fn the_newest_spot_comes_first() {
        let mut s = EditSpots::default();
        s.record(100);
        s.record(5000);
        assert_eq!(s.next(0), Some(5000));
    }

    /// **같은 줄을 고치는 동안 목록이 그 줄로 가득 차면 안 된다.**
    #[test]
    fn nearby_edits_collapse_into_one_spot() {
        let mut s = EditSpots::default();
        for p in 1000..1010 {
            s.record(p);
        }
        s.record(9000);
        // 두 자리만 남아야 한다: 9000과 1000언저리.
        assert_eq!(s.next(0), Some(9000));
        assert!(s.next(9000).is_some_and(|p| (1000..1010).contains(&p)));
    }

    /// **이미 그 자리에 있으면 건너뛴다** — 눌렀는데 안 움직이면 고장으로 읽힌다.
    #[test]
    fn being_there_already_moves_on_to_the_next() {
        let mut s = EditSpots::default();
        s.record(100);
        s.record(9000);
        assert_eq!(s.next(9000), Some(100), "제자리에 머물렀다");
    }

    /// 자리가 하나뿐이고 거기 서 있으면 갈 곳이 없다(그렇다고 헤매지 않는다).
    #[test]
    fn one_spot_you_are_standing_on_gives_nothing() {
        let mut s = EditSpots::default();
        s.record(500);
        assert_eq!(s.next(500), None);
    }

    /// 여러 자리를 **돌아가며** 준다.
    #[test]
    fn repeated_presses_cycle_through_the_spots() {
        let mut s = EditSpots::default();
        for p in [1000, 5000, 9000] {
            s.record(p);
        }
        let a = s.next(0).unwrap();
        let b = s.next(0).unwrap();
        let c = s.next(0).unwrap();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_eq!(s.next(0).unwrap(), a, "한 바퀴 돌지 않았다");
    }

    /// 글이 줄어들면 문서 밖을 가리키는 자리는 버린다.
    #[test]
    fn spots_past_the_end_are_dropped() {
        let mut s = EditSpots::default();
        s.record(9000);
        s.record(100);
        s.clamp(500);
        assert_eq!(s.next(0), Some(100));
        assert_eq!(s.next(100), None, "문서 밖 자리가 남았다");
    }

    /// 오래된 것부터 버린다.
    #[test]
    fn the_list_stops_growing() {
        let mut s = EditSpots::default();
        for i in 0..50 {
            s.record(i * 1000);
        }
        // 여덟 자리만 남는다 — 도는 데 한 바퀴가 여덟 번이어야 한다.
        let mut seen = Vec::new();
        for _ in 0..8 {
            seen.push(s.next(usize::MAX).unwrap());
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 8);
    }
}
