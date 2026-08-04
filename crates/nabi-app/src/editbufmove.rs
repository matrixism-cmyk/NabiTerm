//! 편집 버퍼(E6) 커서 이동·선택 확장 + 들여쓰기 삽입. 변경 연산은 editbufedit.
//!
//! 가로 이동은 grapheme cluster, 세로 이동은 **표시 열** 기준이다(탭·넓은 글자 반영).
//! 모든 이동은 undo 묶음 경계 — 이동 후 타자는 새 묶음에서 시작한다.

use crate::editbuf::EditBuf;

impl EditBuf {
    /// 커서를 새 위치로(select=true면 선택 확장). 이동은 undo 묶음 경계.
    pub(crate) fn move_to(&mut self, pos: usize, select: bool) {
        let pos = pos.min(self.rope.len_chars());
        if select {
            self.move_head(pos); // 고정단(anchor)은 그대로 — 선택 확장.
        } else {
            self.set_cursor(pos);
        }
        self.ensure_visible = true;
        self.undo_open = false;
    }

    /// 좌우 이동(dx=±1). grapheme 단위 — 결합 문자·이모지 안에 커서가 들어가지 않는다.
    pub(crate) fn move_h(&mut self, dx: i64, select: bool) {
        let pos = match dx {
            -1 => self.step_left(),
            1 => self.step_right(),
            _ => (self.cursor() as i64 + dx).clamp(0, self.rope.len_chars() as i64) as usize,
        };
        self.move_to(pos, select);
    }

    /// 상하 이동(dy 줄). **표시 열**을 유지한다 — 탭·넓은 글자가 있어도 시각적으로 같은 자리.
    pub(crate) fn move_v(&mut self, dy: i64, select: bool) {
        let (line, off) = self.cursor_line_col();
        let col = self.disp_line(line).col(off);
        let last = self.rope.len_lines().saturating_sub(1);
        let target = (line as i64 + dy).clamp(0, last as i64) as usize;
        let pos = self.rope.line_to_char(target) + self.disp_line(target).src_at_col(col);
        self.move_to(pos, select);
    }

    /// 줄 처음으로.
    pub(crate) fn home(&mut self, select: bool) {
        let (line, _) = self.cursor_line_col();
        self.move_to(self.rope.line_to_char(line), select);
    }

    /// 줄 끝으로.
    pub(crate) fn end(&mut self, select: bool) {
        let (line, _) = self.cursor_line_col();
        self.move_to(self.rope.line_to_char(line) + self.line_len(line), select);
    }

    /// 전체 선택(undo 묶음 경계).
    pub(crate) fn select_all(&mut self) {
        self.sel = crate::editsel::Selection::single(0, self.rope.len_chars());
        self.undo_open = false;
    }

    /// 단어 단위 이동(Ctrl+←/→). select=true면 선택 확장.
    pub(crate) fn move_word(&mut self, right: bool, select: bool) {
        let pos = if right { self.word_right() } else { self.word_left() };
        self.move_to(pos, select);
    }

    /// Tab 키 — 설정에 따라 탭 문자 또는 다음 탭 스톱까지의 공백을 넣는다.
    pub(crate) fn insert_indent(&mut self) {
        if !self.spaces {
            self.insert("\t");
            return;
        }
        let (line, off) = self.cursor_line_col();
        let col = self.disp_line(line).col(off);
        let n = nabi_types::tab_stop(col, self.tab) - col;
        self.insert(&" ".repeat(n));
    }
}

#[cfg(test)]
mod tests {
    use crate::editbuf::EditBuf;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "LF")
    }

    #[test]
    fn vertical_move_clamps_column() {
        let mut b = buf("abcd\nef\nghij");
        b.set_cursor(3);
        b.move_v(1, false);
        assert_eq!(b.cursor_line_col(), (1, 2));
    }

    #[test]
    fn home_end() {
        let mut b = buf("hi\nworld");
        b.set_cursor(6);
        b.home(false);
        assert_eq!(b.cursor_line_col(), (1, 0));
        b.end(false);
        assert_eq!(b.cursor_line_col(), (1, 5));
    }

    #[test]
    fn arrow_moves_by_grapheme() {
        let mut b = buf("e\u{0301}z");
        b.set_cursor(0);
        b.move_h(1, false);
        assert_eq!(b.cursor(), 2, "결합 악센트 안으로 들어가지 않는다");
        b.move_h(-1, false);
        assert_eq!(b.cursor(), 0);
    }

    #[test]
    fn tab_key_follows_indent_setting() {
        let mut b = buf("ab");
        b.set_cursor(2);
        b.insert_indent(); // 공백 모드 기본 — 2열에서 다음 스톱(4)까지 2칸.
        assert_eq!(b.rope.to_string(), "ab  ");
        b.spaces = false;
        b.insert_indent();
        assert_eq!(b.rope.to_string(), "ab  \t");
    }

    #[test]
    fn vertical_move_keeps_display_column_over_tab() {
        // 위 줄의 탭은 4열까지 펼쳐진다 — 아래 줄에서도 4열(=문자 4번째)로 가야 한다.
        let mut b = buf("\tX\nabcdef");
        b.set_cursor(1); // 탭 바로 뒤 = 4열.
        b.move_v(1, false);
        assert_eq!(b.cursor_line_col(), (1, 4));
    }

    #[test]
    fn select_all_covers_document() {
        let mut b = buf("ab\ncd");
        b.select_all();
        assert_eq!(b.selection(), Some((0, 5)));
    }
}
