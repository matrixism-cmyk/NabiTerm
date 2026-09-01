//! 스크롤백 검색 — 한 줄씩 스크롤하며 보이는 가장자리 줄에서 일치를 찾는다(Find F3).
//! 매칭 판정은 호출자가 술어(pred)로 주입한다(리터럴 스마트케이스 또는 정규식, B7).
//!
//! ## 못 찾으면 **원래 자리로 되돌린다**
//!
//! 예전에는 못 찾아도 스크롤한 자리에 그대로 두고 `false` 를 돌려줬는데, 부르는 쪽이
//! 그 값을 버리고 있었다. 그래서 없는 낱말을 찾으면 **화면이 수천 줄 위로 튀고 아무 말도
//! 없었다.** 사용자가 보기에는 찾기가 아니라 고장이다(2026-09-01 발견).
//!
//! 찾기는 보는 일이지 옮기는 일이 아니다. 찾았을 때만 옮기고, 못 찾았으면 있던 자리에
//! 그대로 둔다.
//!
//! ## 왜 `bool` 이 아니라 열거형인가
//!
//! "없다"와 "상한까지만 봤다"는 사용자에게 전혀 다른 말이다. 앞은 그만 찾으면 되고,
//! 뒤는 상한을 올리면 나올 수도 있다. 둘을 `false` 하나로 뭉개면 그 차이를 말해 줄 수 없다.
//!
//! 반환 타입을 바꾼 덕에 **부르는 쪽이 값을 버리면 컴파일이 깨진다** — 예전 결함이 바로
//! 그 버림이었으므로, 같은 일이 다시 일어나지 않게 타입으로 막았다.

use crate::grid::TermModel;

/// 한 번 훑은 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "못 찾았는지 상한에 걸렸는지 사용자에게 말해 줘야 한다"]
pub enum MatchScan {
    /// 찾았다 — 화면을 그 줄로 옮겼다.
    Found,
    /// 스크롤백 끝까지 봤지만 없다. 화면은 있던 자리로 되돌렸다.
    NotFound,
    /// 상한(`max_steps`)까지 봤는데 아직 끝이 아니다. 화면은 되돌렸다.
    HitLimit,
}

impl TermModel {
    /// 위로 스크롤하며 pred에 맞는 더 오래된 줄로 이동한다(맨 윗줄 검사).
    pub fn scroll_to_prev_match(&mut self, pred: impl Fn(&str) -> bool, max_steps: usize) -> MatchScan {
        let start = self.scrollback_offset();
        for _ in 0..max_steps {
            let before = self.scrollback_offset();
            self.scroll_by(1);
            if self.scrollback_offset() == before {
                self.restore_scroll(start);
                return MatchScan::NotFound; // 맨 위까지 봤다.
            }
            self.mark_dirty();
            if pred(&self.visible_row_text(0)) {
                return MatchScan::Found;
            }
        }
        self.restore_scroll(start);
        MatchScan::HitLimit
    }

    /// 아래로 스크롤하며 pred에 맞는 더 최신 줄로 이동한다(맨 아랫줄 검사).
    pub fn scroll_to_next_match(&mut self, pred: impl Fn(&str) -> bool, max_steps: usize) -> MatchScan {
        let start = self.scrollback_offset();
        let last = self.size().rows().saturating_sub(1);
        for _ in 0..max_steps {
            if self.scrollback_offset() == 0 {
                self.restore_scroll(start);
                return MatchScan::NotFound; // 이미 하단까지 봤다.
            }
            self.scroll_by(-1);
            self.mark_dirty();
            if pred(&self.visible_row_text(last)) {
                return MatchScan::Found;
            }
        }
        self.restore_scroll(start);
        MatchScan::HitLimit
    }

    /// 훑기 전 자리로 되돌린다. 바닥에서 세어 올라가는 것이 유일하게 확실한 길이다 —
    /// 훑는 동안 새 출력이 들어와 상대 이동이 어긋났을 수 있기 때문이다.
    fn restore_scroll(&mut self, offset: usize) {
        self.scroll_to_bottom();
        if offset > 0 {
            self.scroll_by(offset as i32);
        }
        self.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::MatchScan;
    use crate::grid::TermModel;
    use nabi_types::GridSize;

    fn model_with_lines(n: usize) -> TermModel {
        let mut m = TermModel::new(GridSize::new(20, 3), 100);
        for i in 1..=n {
            m.process(format!("line{i}\r\n").as_bytes());
        }
        m
    }

    #[test]
    fn finds_older_line_scrolling_up() {
        let mut m = model_with_lines(10); // line1..line10 — 오래된 줄은 스크롤백으로.
        assert_eq!(m.scroll_to_prev_match(|t| t.contains("line2"), 50), MatchScan::Found);
    }

    #[test]
    fn missing_pattern_says_not_found() {
        let mut m = model_with_lines(10);
        assert_eq!(m.scroll_to_prev_match(|t| t.contains("zzz"), 50), MatchScan::NotFound);
    }

    /// **못 찾았으면 화면이 움직이면 안 된다.** 예전에는 수천 줄 위로 튄 채 아무 말도
    /// 없었다 — 이 시험이 그 결함을 지킨다.
    #[test]
    fn a_failed_search_leaves_the_view_where_it_was() {
        let mut m = model_with_lines(30);
        m.scroll_by(4); // 사용자가 네 줄 위를 보고 있다.
        let before = m.scrollback_offset();
        assert_eq!(m.scroll_to_prev_match(|t| t.contains("zzz"), 200), MatchScan::NotFound);
        assert_eq!(m.scrollback_offset(), before, "못 찾았는데 화면을 옮겼다");
    }

    /// **상한에 걸린 것과 없는 것은 다른 말이다** — 앞은 상한을 올리면 나온다.
    /// 같은 낱말을 상한만 바꿔 두 번 찾아 그 차이를 못 박는다.
    #[test]
    fn hitting_the_limit_is_not_the_same_as_being_absent() {
        let needle = |t: &str| t.contains("needle");
        let mut m = TermModel::new(GridSize::new(20, 3), 100);
        m.process(b"needle\r\n");
        for i in 1..=60 {
            m.process(format!("filler{i}\r\n").as_bytes());
        }
        assert_eq!(m.scroll_to_prev_match(needle, 3), MatchScan::HitLimit, "덜 본 것을 없다고 했다");
        assert_eq!(m.scroll_to_prev_match(needle, 200), MatchScan::Found, "상한을 올리면 나와야 한다");
    }

    /// 상한에 걸렸을 때도 화면은 되돌아온다.
    #[test]
    fn hitting_the_limit_also_restores_the_view() {
        let mut m = model_with_lines(60);
        let before = m.scrollback_offset();
        assert_eq!(m.scroll_to_prev_match(|_| false, 3), MatchScan::HitLimit);
        assert_eq!(m.scrollback_offset(), before);
    }

    /// 아래 방향도 같은 규칙이다.
    #[test]
    fn searching_downwards_restores_too() {
        let mut m = model_with_lines(30);
        m.scroll_by(10);
        let before = m.scrollback_offset();
        assert_eq!(m.scroll_to_next_match(|t| t.contains("zzz"), 200), MatchScan::NotFound);
        assert_eq!(m.scrollback_offset(), before);
    }
}
