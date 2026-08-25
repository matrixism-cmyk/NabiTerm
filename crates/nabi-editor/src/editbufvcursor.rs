//! **위/아래로 커서 늘리기** — Ctrl+Alt+↑/↓.
//!
//! 지난 배치에서 "같은 낱말을 하나씩 더 잡기"(Ctrl+D)를 냈는데, 다중 커서에는 짝이 되는
//! 방식이 하나 더 있다. 표처럼 줄 맞춰 놓인 것을 고칠 때는 **같은 열의 위아래**를 잡는다.
//! VS Code·Sublime이 둘 다 갖고 있고, 둘은 쓰임이 겹치지 않는다.
//!
//! ## 열을 어떻게 잡는가
//!
//! 같은 **표시 열**을 쓴다(탭이 있으면 글자 수와 화면 위치가 다르다). 짧은 줄에서는 줄 끝에
//! 놓는다 — 잘라 내는 대신 끝에 붙여야 타자를 쳤을 때 모든 줄에 들어간다(박스 선택과 같은
//! 규칙, `editbufboxsel` 참고).

use crate::editbuf::EditBuf;
use crate::editsel::Range;

impl EditBuf {
    /// 주 커서의 위(`-1`) 또는 아래(`+1`) 줄에 커서를 하나 더 놓는다.
    ///
    /// 더 갈 줄이 없으면 아무것도 하지 않고 false. 이미 그 줄에 커서가 있으면 그대로 둔다
    /// (선택 모델이 병합하므로 개수가 줄어든 것처럼 보이는 것을 막는다).
    pub fn add_cursor_vertical(&mut self, dy: i64) -> bool {
        let p = self.sel.primary();
        let col = self.display_col_at(p.head);
        // 기준은 **가장 바깥 커서**다 — 위로 늘릴 때는 가장 위, 아래로는 가장 아래에서 이어야
        // 누를 때마다 한 줄씩 뻗는다. 주 커서에서만 재면 두 번째부터 제자리걸음이 된다.
        let lines: Vec<usize> = self.sel.ranges().iter().map(|r| self.rope.char_to_line(r.head)).collect();
        let from = match dy < 0 {
            true => *lines.iter().min().unwrap_or(&0),
            false => *lines.iter().max().unwrap_or(&0),
        };
        let Some(next) = from.checked_add_signed(dy as isize) else { return false };
        if next >= self.rope.len_lines() {
            return false;
        }
        let at = self.char_at_display_col(next, col);
        if lines.contains(&next) {
            return false;
        }
        self.sel.push(Range::caret(at));
        true
    }

    /// 그 위치의 표시 열(탭은 다음 탭 자리까지).
    fn display_col_at(&self, at: usize) -> usize {
        let line = self.rope.char_to_line(at.min(self.rope.len_chars()));
        let start = self.rope.line_to_char(line);
        let mut col = 0usize;
        for c in self.rope.slice(start..at).chars() {
            col = advance(col, c);
        }
        col
    }

    /// 그 줄에서 이 표시 열에 해당하는 문자 위치. 줄이 짧으면 줄 끝.
    fn char_at_display_col(&self, line: usize, want: usize) -> usize {
        let start = self.rope.line_to_char(line);
        let mut col = 0usize;
        let mut at = start;
        for c in self.rope.line(line).chars() {
            if c == '\n' || col >= want {
                break;
            }
            col = advance(col, c);
            at += 1;
        }
        at
    }
}

/// 탭 폭 — 화면 열 계산용(에디터 기본과 같다).
const TAB: usize = 4;

fn advance(col: usize, c: char) -> usize {
    match c {
        '\t' => col + TAB - (col % TAB),
        _ => col + 1,
    }
}
