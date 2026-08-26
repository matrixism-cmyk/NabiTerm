//! **명령 블록 목록** — 지나간 명령을 한자리에 모아 보여 주고, 눌러서 그 자리로 간다.
//!
//! 프롬프트 표식(OSC 133)은 이미 있다. 없던 것은 **훑는 길**이다. 긴 로그에서 "아까 그
//! 명령"을 되찾으려면 지금은 스크롤을 되짚어야 한다. 표식마다 그 줄의 글자와 종료 코드가
//! 이미 손에 있으므로, 모아 주기만 하면 목록이 된다.
//!
//! 여기서는 **읽기만** 한다 — 화면을 옮기는 일은 `scroll_to_prompt` 하나뿐이고, 그것도
//! 이미 있는 오프셋 계산을 그대로 쓴다.

use crate::grid::TermModel;

/// 목록에 보일 블록 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInfo {
    /// 프롬프트가 있는 절대 줄(그 자리로 갈 때 쓴다).
    pub abs: i64,
    /// 종료 코드. `None`이면 아직 안 끝났거나 셸이 알려 주지 않았다.
    pub exit: Option<i32>,
    /// 프롬프트 줄의 글자 — 대개 "프롬프트 + 친 명령"이다.
    pub text: String,
    /// 이 블록이 뱉은 출력 줄 수(다음 프롬프트까지). 마지막 블록은 화면 끝까지.
    pub out_lines: usize,
}

impl TermModel {
    /// 기록된 블록을 **최신이 앞**에 오도록 돌려준다.
    ///
    /// 최신이 앞인 이유: 되찾고 싶은 것은 거의 항상 방금 것이다. 목록을 열자마자 위에
    /// 있어야 한다.
    pub fn command_blocks(&self) -> Vec<BlockInfo> {
        // 커서가 앉은 마지막 빈 줄은 출력이 아니다 — 세면 마지막 블록만 한 줄씩 부풀어
        // 보인다(다른 블록은 다음 프롬프트에서 끊기므로 이 문제가 없다).
        let mut total = self.total_abs_lines() as i64;
        if total > 0 {
            let last = self.lines_abs_text(total as usize - 1, total as usize);
            if last.first().is_none_or(|l| l.trim().is_empty()) {
                total -= 1;
            }
        }
        let marks = &self.prompts;
        let mut out = Vec::with_capacity(marks.len());
        for (i, p) in marks.iter().enumerate() {
            let next = marks.get(i + 1).map(|n| n.abs).unwrap_or(total);
            let line = self.lines_abs_text(p.abs.max(0) as usize, (p.abs + 1).max(0) as usize);
            let text = line.first().map(|s| s.trim_end().to_string()).unwrap_or_default();
            // 프롬프트 줄 자신은 출력이 아니다 — 빼고 센다.
            let out_lines = (next - p.abs - 1).max(0) as usize;
            out.push(BlockInfo { abs: p.abs, exit: p.exit, text, out_lines });
        }
        out.reverse();
        out
    }

    /// 그 블록이 **화면 맨 위**에 오게 스크롤한다. 이미 거기면 false.
    pub fn scroll_to_prompt(&mut self, abs: i64) -> bool {
        let cur = self.scrollback_offset() as i32;
        let d = self.prompt_offset(abs);
        if d == cur {
            return false;
        }
        self.scroll_by(d - cur);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::TermModel;
    use nabi_types::GridSize;

    fn run(m: &mut TermModel, cmd: &str, code: i32, out: usize) {
        m.mark_prompt();
        m.process(format!("$ {cmd}\r\n").as_bytes());
        for i in 0..out {
            m.process(format!("out {i}\r\n").as_bytes());
        }
        m.mark_command_done(Some(code));
    }

    /// 목록은 **최신이 앞**이고, 명령 글자와 종료 코드를 함께 준다.
    #[test]
    fn the_newest_command_comes_first() {
        let mut m = TermModel::new(GridSize::new(60, 6), 500);
        run(&mut m, "first", 0, 2);
        run(&mut m, "second", 1, 3);
        let b = m.command_blocks();
        assert_eq!(b.len(), 2);
        assert!(b[0].text.contains("second"), "{:?}", b[0].text);
        assert_eq!(b[0].exit, Some(1));
        assert!(b[1].text.contains("first"));
        assert_eq!(b[1].exit, Some(0));
    }

    /// 출력 줄 수를 센다 — 목록에서 "긴 명령"을 알아보는 유일한 단서다.
    #[test]
    fn each_block_knows_how_much_it_printed() {
        let mut m = TermModel::new(GridSize::new(60, 6), 500);
        run(&mut m, "quiet", 0, 1);
        run(&mut m, "loud", 0, 7);
        let b = m.command_blocks();
        assert_eq!(b[0].out_lines, 7, "마지막 블록을 화면 끝까지 세지 못했다");
        assert_eq!(b[1].out_lines, 1);
    }

    /// 아직 안 끝난 명령은 코드가 **없다** — 0으로 꾸미면 실패를 성공으로 보이게 한다.
    #[test]
    fn a_running_command_has_no_code_yet() {
        let mut m = TermModel::new(GridSize::new(60, 6), 300);
        m.mark_prompt();
        m.process(b"$ sleep 100\r\n");
        assert_eq!(m.command_blocks()[0].exit, None);
    }

    /// 눌러서 그 자리로 간다 — 고른 블록이 화면 맨 위에 온다.
    #[test]
    fn picking_a_block_scrolls_to_it() {
        let mut m = TermModel::new(GridSize::new(60, 5), 500);
        run(&mut m, "ALPHA", 0, 8);
        run(&mut m, "BETA", 0, 8);
        let target = m.command_blocks().last().expect("블록이 없다").abs;
        assert!(m.scroll_to_prompt(target));
        assert!(m.visible_row_text(0).contains("ALPHA"), "{}", m.visible_row_text(0));
        // 같은 자리로 또 가면 움직일 것이 없다.
        assert!(!m.scroll_to_prompt(target));
    }

    /// 블록이 없으면 빈 목록이다(없는 것을 꾸며 내지 않는다).
    #[test]
    fn no_prompts_means_no_blocks() {
        let m = TermModel::new(GridSize::new(40, 5), 100);
        assert!(m.command_blocks().is_empty());
    }
}
