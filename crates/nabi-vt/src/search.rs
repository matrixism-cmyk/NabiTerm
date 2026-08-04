//! 스크롤백 검색 — 한 줄씩 스크롤하며 보이는 가장자리 줄에서 일치를 찾는다(Find F3).
//! 매칭 판정은 호출자가 술어(pred)로 주입한다(리터럴 스마트케이스 또는 정규식, B7).

use crate::grid::TermModel;

impl TermModel {
    /// 위로 스크롤하며 pred에 맞는 더 오래된 줄로 이동한다(맨 윗줄 검사). 찾으면 true.
    pub fn scroll_to_prev_match(&mut self, pred: impl Fn(&str) -> bool, max_steps: usize) -> bool {
        for _ in 0..max_steps {
            let before = self.scrollback_offset();
            self.scroll_by(1);
            if self.scrollback_offset() == before {
                return false; // 맨 위.
            }
            self.mark_dirty();
            if pred(&self.visible_row_text(0)) {
                return true;
            }
        }
        false
    }

    /// 아래로 스크롤하며 pred에 맞는 더 최신 줄로 이동한다(맨 아랫줄 검사). 찾으면 true.
    pub fn scroll_to_next_match(&mut self, pred: impl Fn(&str) -> bool, max_steps: usize) -> bool {
        let last = self.size().rows().saturating_sub(1);
        for _ in 0..max_steps {
            if self.scrollback_offset() == 0 {
                return false; // 이미 하단.
            }
            self.scroll_by(-1);
            self.mark_dirty();
            if pred(&self.visible_row_text(last)) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
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
        assert!(m.scroll_to_prev_match(|t| t.contains("line2"), 50)); // 위로 스크롤해 발견.
    }

    #[test]
    fn missing_pattern_returns_false() {
        let mut m = model_with_lines(10);
        assert!(!m.scroll_to_prev_match(|t| t.contains("zzz"), 50)); // 없으면 맨 위까지 가고 false.
    }
}
