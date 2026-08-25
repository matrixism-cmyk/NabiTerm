//! **스크롤백 표식** — 긴 로그에서 되짚을 자리를 남긴다.
//!
//! 긴 작업을 지켜볼 때 하는 일은 늘 같다. "배포 시작한 자리"를 봐 두고, 한참 흘러간 뒤
//! 거기로 되돌아가 무엇이 달라졌는지 본다. 지금까지 그 자리를 되찾는 길은 찾기(Ctrl+F)뿐이라,
//! 되찾으려면 **그 줄에 무슨 글자가 있었는지 기억해야** 했다.
//!
//! 표식은 그 기억을 대신한다. 지금 보는 자리에 표를 남기고, 표 사이를 오간다.
//!
//! ## 왜 절대 줄 번호인가
//!
//! 화면 위치가 아니라 스크롤백의 절대 줄 번호로 잡는다. 출력이 계속 흘러도 표식이 가리키는
//! 줄은 그대로다. 다만 스크롤백이 상한을 넘겨 앞부분이 잘려 나가면 그 표식은 **가리킬 곳이
//! 없어진다** — 그때는 조용히 지운다(엉뚱한 줄로 보내는 것보다 낫다).

/// pane 하나의 표식들. 절대 줄 번호, 오름차순·중복 없음.
#[derive(Default, Clone, Debug)]
pub(crate) struct Marks {
    lines: Vec<usize>,
}

/// 한 pane이 가질 수 있는 표식 수. 넘으면 가장 오래된 것부터 버린다.
pub(crate) const MAX: usize = 64;

impl Marks {
    /// 그 줄의 표식을 켜거나 끈다. 켜졌으면 true.
    pub(crate) fn toggle(&mut self, line: usize) -> bool {
        match self.lines.binary_search(&line) {
            Ok(i) => {
                self.lines.remove(i);
                false
            }
            Err(i) => {
                self.lines.insert(i, line);
                // 상한을 넘으면 **가장 위(오래된) 것**을 버린다 — 최근 자리가 더 쓸모 있다.
                if self.lines.len() > MAX {
                    self.lines.remove(0);
                }
                true
            }
        }
    }

    /// `from`보다 **아래**(더 최근)의 첫 표식.
    pub(crate) fn next_after(&self, from: usize) -> Option<usize> {
        self.lines.iter().copied().find(|&l| l > from)
    }

    /// `from`보다 **위**(더 오래된)의 첫 표식.
    pub(crate) fn prev_before(&self, from: usize) -> Option<usize> {
        self.lines.iter().rev().copied().find(|&l| l < from)
    }

    /// 스크롤백에서 잘려 나간 표식을 버린다. `first_abs` = 지금 남아 있는 첫 줄 번호.
    ///
    /// 가리킬 곳이 없어진 표식을 남겨 두면 눌렀을 때 엉뚱한 자리로 간다.
    pub(crate) fn drop_trimmed(&mut self, first_abs: usize) {
        self.lines.retain(|&l| l >= first_abs);
    }

    pub(crate) fn all(&self) -> &[usize] {
        &self.lines
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Marks, MAX};

    #[test]
    fn a_mark_toggles_on_and_off() {
        let mut m = Marks::default();
        assert!(m.toggle(10));
        assert_eq!(m.all(), [10]);
        assert!(!m.toggle(10));
        assert!(m.is_empty());
    }

    /// 표식은 늘 정렬돼 있어야 한다 — 오가는 순서가 화면 순서와 같아야 하기 때문이다.
    #[test]
    fn marks_stay_in_order_however_they_were_added() {
        let mut m = Marks::default();
        for l in [50, 10, 30] {
            m.toggle(l);
        }
        assert_eq!(m.all(), [10, 30, 50]);
    }

    #[test]
    fn moving_between_marks_finds_the_neighbours() {
        let mut m = Marks::default();
        for l in [10, 30, 50] {
            m.toggle(l);
        }
        assert_eq!(m.next_after(10), Some(30));
        assert_eq!(m.prev_before(50), Some(30));
        assert_eq!(m.next_after(50), None, "마지막 아래로는 갈 곳이 없다");
        assert_eq!(m.prev_before(10), None);
    }

    /// 지금 자리에 표식이 있어도 **그 자리에 머물지 않는다** — 눌렀는데 안 움직이면 고장이다.
    #[test]
    fn navigation_never_lands_on_where_you_already_are() {
        let mut m = Marks::default();
        m.toggle(30);
        assert_eq!(m.next_after(30), None);
        assert_eq!(m.prev_before(30), None);
    }

    /// **잘려 나간 표식은 지운다.** 남겨 두면 눌렀을 때 엉뚱한 줄로 간다.
    #[test]
    fn marks_scrolled_out_of_history_are_dropped() {
        let mut m = Marks::default();
        for l in [10, 30, 50] {
            m.toggle(l);
        }
        m.drop_trimmed(25);
        assert_eq!(m.all(), [30, 50]);
    }

    /// 상한을 넘으면 오래된 것부터 버린다 — 최근 자리가 더 쓸모 있다.
    #[test]
    fn the_oldest_mark_gives_way_when_full() {
        let mut m = Marks::default();
        for l in 0..MAX + 3 {
            m.toggle(l * 10);
        }
        assert_eq!(m.all().len(), MAX);
        assert_eq!(m.all()[0], 30, "가장 오래된 셋이 빠져야 한다");
    }

    #[test]
    fn clearing_removes_everything() {
        let mut m = Marks::default();
        m.toggle(1);
        m.clear();
        assert!(m.is_empty());
    }
}
