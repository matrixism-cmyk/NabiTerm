//! 명령 블록(OSC 133) — 프롬프트 절대 줄 + 종료코드를 기록해 점프/블록 바(Warp식)에 쓴다.

use crate::grid::TermModel;

/// 한 명령 블록 표식: 프롬프트 시작 절대 줄번호 + 명령 종료코드(아직이면 None).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptMark {
    pub abs: i64,
    pub exit: Option<i32>,
    /// 이 프롬프트가 찍힌 시각 — 명령이 끝나면 여기서 걸린 시간을 잰다.
    ///
    /// 시각을 따로 두지 않고 마크에 얹은 까닭: 프롬프트는 겹쳐 찍힐 수 있고(화면을 지우고
    /// 다시 그리는 TUI), 그때 "마지막 하나"만 들고 있으면 짝이 어긋난다.
    pub started: Option<std::time::Instant>,
    /// 명령이 걸린 시간(밀리초). 끝나야 채워진다.
    pub ms: Option<u64>,
}

impl TermModel {
    /// 프롬프트 시작 시점을 기록한다(orchestrator가 OSC 133;A 검출 시 호출).
    /// 절대 줄번호 = 현재 히스토리 길이 + 커서 행.
    pub fn mark_prompt(&mut self) {
        let abs = self.history_size() as i64 + i64::from(self.cursor_line());
        if self.prompts.last().map(|p| p.abs) != Some(abs) {
            let started = Some(std::time::Instant::now());
            self.prompts.push(PromptMark { abs, exit: None, started, ms: None });
        }
        if self.prompts.len() > 1000 {
            self.prompts.remove(0); // 무한 성장 방지.
        }
    }

    /// 명령 종료(OSC 133;D[;code])를 마지막 프롬프트 블록에 기록한다(블록 바 색상용).
    pub fn mark_command_done(&mut self, code: Option<i32>) {
        if let Some(last) = self.prompts.last_mut() {
            last.exit = code;
            // 시각이 없으면(우리가 못 본 프롬프트) 0으로 꾸미지 않고 비워 둔다.
            last.ms = last.started.map(|t| t.elapsed().as_millis() as u64);
        }
    }

    /// 마지막으로 완료된 명령의 출력 텍스트(Warp식 블록 복사, C4). 인접 두 프롬프트 사이
    /// (이전 프롬프트의 명령줄 다음 ~ 다음 프롬프트 직전)를 추출. 프롬프트가 1개면 하단까지.
    pub fn last_command_output(&self) -> Option<String> {
        let n = self.prompts.len();
        if n == 0 {
            return None;
        }
        let (start, end) = if n >= 2 {
            (self.prompts[n - 2].abs + 1, self.prompts[n - 1].abs)
        } else {
            (self.prompts[n - 1].abs + 1, self.total_abs_lines() as i64)
        };
        if end <= start {
            return None;
        }
        let lines = self.lines_abs_text(start.max(0) as usize, end.max(0) as usize);
        let mut s = lines.join("\n");
        while s.ends_with('\n') || s.ends_with(' ') {
            s.pop();
        }
        (!s.trim().is_empty()).then_some(s)
    }

    /// 현재 화면에 보이는 프롬프트들의 (행, 종료코드)를 돌려준다 — 좌측 블록 바 렌더용.
    /// 보이는 행 = abs − history_size + scrollback_offset (0..rows 범위면 화면 안).
    pub fn visible_prompt_rows(&self) -> Vec<(u16, Option<i32>)> {
        let h = self.history_size() as i64;
        let d = self.scrollback_offset() as i64;
        let rows = i64::from(self.size().rows());
        self.prompts
            .iter()
            .filter_map(|p| {
                let row = p.abs - h + d;
                (row >= 0 && row < rows).then_some((row as u16, p.exit))
            })
            .collect()
    }

    /// 기록된 프롬프트를 화면 맨 위에 보이게 할 display offset(히스토리 캡 시 클램프).
    pub(crate) fn prompt_offset(&self, abs: i64) -> i32 {
        let h = self.history_size() as i64;
        (h - abs).clamp(0, h) as i32
    }

    /// 더 오래된(위쪽) 프롬프트로 점프. 이동했으면 true.
    /// **실패한 명령으로만** 오간다(종료 코드가 0이 아닌 것).
    ///
    /// 긴 작업 뒤에 알고 싶은 것은 "무엇이 깨졌나"다. 프롬프트를 하나씩 되짚으면 성공한
    /// 명령 오십 개를 지나야 그 하나에 닿는다. 종료 코드는 이미 블록에 있으므로,
    /// 거르는 것만 더하면 된다.
    ///
    /// `back`이면 더 과거로, 아니면 더 최신으로. 이동했으면 true.
    pub fn jump_failed_prompt(&mut self, back: bool) -> bool {
        let cur = self.scrollback_offset() as i32;
        let offs = self
            .prompts
            .iter()
            .filter(|p| p.exit.is_some_and(|c| c != 0))
            .map(|p| self.prompt_offset(p.abs));
        // 오프셋은 "최신에서 얼마나 거슬러 올라갔나"라 과거일수록 크다.
        let target = match back {
            true => offs.filter(|&d| d > cur).min(),
            false => offs.filter(|&d| d < cur).max(),
        };
        match target {
            Some(d) => {
                self.scroll_by(d - cur);
                true
            }
            None => false,
        }
    }

    /// 실패한 명령이 몇 개나 있나(화면에 개수를 보여 주려면 필요하다).
    pub fn failed_count(&self) -> usize {
        self.prompts.iter().filter(|p| p.exit.is_some_and(|c| c != 0)).count()
    }

    #[must_use = "못 갔으면 사용자에게 말해 줘야 한다 -- 조용하면 고장으로 읽힌다"]
    pub fn jump_prev_prompt(&mut self) -> bool {
        let cur = self.scrollback_offset() as i32;
        let target = self
            .prompts
            .iter()
            .map(|p| self.prompt_offset(p.abs))
            .filter(|&d| d > cur)
            .min();
        match target {
            Some(d) => {
                self.scroll_by(d - cur);
                true
            }
            None => false,
        }
    }

    /// 더 최신(아래쪽) 프롬프트로 점프(없으면 하단 복귀). 이동했으면 true.
    #[must_use = "못 갔으면 사용자에게 말해 줘야 한다 -- 조용하면 고장으로 읽힌다"]
    pub fn jump_next_prompt(&mut self) -> bool {
        let cur = self.scrollback_offset() as i32;
        let target = self
            .prompts
            .iter()
            .map(|p| self.prompt_offset(p.abs))
            .filter(|&d| d < cur)
            .max();
        match target {
            Some(d) => {
                self.scroll_by(d - cur);
                true
            }
            None => {
                let was = cur != 0;
                self.scroll_to_bottom();
                was
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::TermModel;
    use nabi_types::GridSize;

    /// **실패한 것만 지나간다** — 성공한 명령 사이를 하나씩 되짚지 않는다.
    #[test]
    fn jumping_by_failures_skips_the_successes() {
        let mut m = TermModel::new(GridSize::new(40, 5), 400);
        // 실패(BAD1) → 성공 셋 → 실패(BAD2) → 꼬리.
        m.mark_prompt();
        m.process(b"BAD1$ boom\r\n");
        m.mark_command_done(Some(1));
        for i in 0..3 {
            for _ in 0..6 {
                m.process(format!("filler {i}\r\n").as_bytes());
            }
            m.mark_prompt();
            m.process(format!("OK{i}$ fine\r\n").as_bytes());
            m.mark_command_done(Some(0));
        }
        for _ in 0..6 {
            m.process(b"more\r\n");
        }
        m.mark_prompt();
        m.process(b"BAD2$ boom again\r\n");
        m.mark_command_done(Some(2));
        for _ in 0..10 {
            m.process(b"tail\r\n");
        }
        assert_eq!(m.failed_count(), 2, "실패를 잘못 셌다");
        // 뒤로 한 번 → 성공 셋을 건너뛰고 BAD2.
        assert!(m.jump_failed_prompt(true));
        assert!(m.visible_row_text(0).contains("BAD2$"), "성공한 것에 멈췄다");
        // 한 번 더 → BAD1.
        assert!(m.jump_failed_prompt(true));
        assert!(m.visible_row_text(0).contains("BAD1$"));
        // 더 위로는 실패가 없다.
        assert!(!m.jump_failed_prompt(true), "없는 곳으로 갔다");
        // 앞으로 → BAD2로 돌아온다.
        assert!(m.jump_failed_prompt(false));
        assert!(m.visible_row_text(0).contains("BAD2$"));
    }

    /// 실패가 하나도 없으면 움직이지 않는다(움직이면 사용자가 자리를 잃는다).
    #[test]
    fn with_no_failures_nothing_moves() {
        let mut m = TermModel::new(GridSize::new(40, 5), 200);
        m.mark_prompt();
        m.process(b"OK$ fine\r\n");
        m.mark_command_done(Some(0));
        for _ in 0..20 {
            m.process(b"x\r\n");
        }
        let before = m.scrollback_offset();
        assert!(!m.jump_failed_prompt(true));
        assert!(!m.jump_failed_prompt(false));
        assert_eq!(m.scrollback_offset(), before, "안 움직여야 하는데 움직였다");
        assert_eq!(m.failed_count(), 0);
    }

    #[test]
    fn jumps_between_prompts() {
        let mut m = TermModel::new(GridSize::new(40, 5), 200);
        m.mark_prompt(); // P1 줄.
        m.process(b"P1$ run\r\n");
        for i in 0..30 {
            m.process(format!("out {i}\r\n").as_bytes());
        }
        m.mark_prompt(); // P2 줄.
        m.process(b"P2$ next\r\n");
        for i in 0..20 {
            m.process(format!("tail {i}\r\n").as_bytes());
        }
        // 이전 프롬프트(가까운 P2)로 → 맨 윗줄에 P2.
        assert!(m.jump_prev_prompt());
        assert!(m.visible_row_text(0).contains("P2$"));
        // 한 번 더 → P1.
        assert!(m.jump_prev_prompt());
        assert!(m.visible_row_text(0).contains("P1$"));
        // 다음 → P2로 복귀.
        assert!(m.jump_next_prompt());
        assert!(m.visible_row_text(0).contains("P2$"));
        // 다음 → 더 없음 → 하단 복귀.
        assert!(m.jump_next_prompt());
        assert_eq!(m.scrollback_offset(), 0);
        assert!(!m.jump_next_prompt()); // 이미 하단.
    }

    #[test]
    fn last_output_between_prompts() {
        let mut m = TermModel::new(GridSize::new(40, 6), 200);
        m.mark_prompt();
        m.process(b"P1$ echo hi\r\n");
        m.process(b"hi\r\nworld\r\n");
        m.mark_command_done(Some(0));
        m.mark_prompt(); // 다음 프롬프트 → 직전 명령 출력 확정.
        let out = m.last_command_output().unwrap();
        assert!(out.contains("hi") && out.contains("world"));
        assert!(!out.contains("P1$")); // 명령줄은 제외
    }

    #[test]
    fn visible_prompt_rows_record_exit_codes() {
        let mut m = TermModel::new(GridSize::new(40, 6), 200);
        m.mark_prompt();
        m.process(b"P1$ run\r\n");
        m.mark_command_done(Some(0)); // 성공.
        let marks = m.visible_prompt_rows();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], (0, Some(0)));
        m.mark_prompt();
        m.process(b"P2$ bad\r\n");
        m.mark_command_done(Some(1)); // 실패.
        let marks = m.visible_prompt_rows();
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0], (0, Some(0)));
        assert_eq!(marks[1], (1, Some(1)));
    }
}
