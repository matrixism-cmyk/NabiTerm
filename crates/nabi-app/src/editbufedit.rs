//! 편집 버퍼(E6) 변경 연산 + undo/redo(연속 편집 묶음 coalescing). editbuf.rs의 EditBuf impl.
//!
//! 같은 종류(삽입/삭제) 연속 편집은 한 undo 단위로 묶어 Ctrl+Z가 글자별이 아니라 구간별로 동작한다.
//! 커서 이동·종류 전환·선택 대체·줄바꿈은 묶음 경계.

use crate::editbuf::{EditBuf, EditKind};

/// undo 스냅샷 최대 개수(rope clone은 구조 공유라 메모리 부담은 작음).
const UNDO_MAX: usize = 1000;

impl EditBuf {
    /// 편집 직전 호출 — 같은 종류 연속이면 묶고(스냅샷 생략), 아니면 새 스냅샷을 쌓는다.
    fn begin(&mut self, kind: EditKind, fresh: bool) {
        if !fresh && self.undo_open && self.last_kind == Some(kind) {
            self.redo.clear(); // 묶음 계속 — 스냅샷 추가 안 함.
        } else {
            let snap = (self.rope.clone(), self.cursor());
            self.undo.push(snap);
            if self.undo.len() > UNDO_MAX {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.undo_open = true;
        }
        self.last_kind = Some(kind);
    }

    /// 선택이 있으면 지우고 캐럿을 그 시작점에 둔다(없으면 선택만 해제 — 이미 캐럿).
    fn del_selection(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.rope.remove(a..b);
            self.set_cursor(a);
        }
    }

    /// 텍스트 삽입(선택이 있으면 대체). 줄바꿈/선택대체는 묶음 경계.
    pub(crate) fn insert(&mut self, s: &str) {
        let fresh = self.selection().is_some() || s.contains('\n');
        self.begin(EditKind::Insert, fresh);
        self.del_selection();
        let at = self.cursor();
        self.rope.insert(at, s);
        self.set_cursor(at + s.chars().count());
        self.dirty = true;
        self.ensure_visible = true;
        if s.contains('\n') {
            self.undo_open = false; // 줄바꿈 후 새 묶음.
        }
    }

    /// Enter — 새 줄 + 현재 줄 선행 공백 복사(자동 들여쓰기).
    pub(crate) fn insert_newline(&mut self) {
        let line = self.cursor_line_col().0;
        let indent: String = self.line_string(line).chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        self.insert(&format!("\n{indent}"));
    }

    /// 백스페이스(선택이 있으면 선택 삭제).
    pub(crate) fn backspace(&mut self) {
        let fresh = self.selection().is_some();
        self.begin(EditKind::Delete, fresh);
        if self.selection().is_some() {
            self.del_selection();
        } else if self.cursor() > 0 {
            let c = self.cursor();
            self.rope.remove(c - 1..c);
            self.set_cursor(c - 1);
        }
        self.dirty = true;
        self.ensure_visible = true;
    }

    /// Delete(앞 글자 삭제, 선택 우선).
    pub(crate) fn delete(&mut self) {
        let fresh = self.selection().is_some();
        self.begin(EditKind::Delete, fresh);
        if self.selection().is_some() {
            self.del_selection();
        } else if self.cursor() < self.rope.len_chars() {
            let c = self.cursor();
            self.rope.remove(c..c + 1);
        }
        self.dirty = true;
        self.ensure_visible = true;
    }

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

    /// 좌우 이동(dx=±1).
    pub(crate) fn move_h(&mut self, dx: i64, select: bool) {
        let pos = (self.cursor() as i64 + dx).clamp(0, self.rope.len_chars() as i64) as usize;
        self.move_to(pos, select);
    }

    /// 상하 이동(dy 줄, 열 유지). 줄 끝 길이로 클램프.
    pub(crate) fn move_v(&mut self, dy: i64, select: bool) {
        let (line, col) = self.cursor_line_col();
        let last = self.rope.len_lines().saturating_sub(1);
        let target = (line as i64 + dy).clamp(0, last as i64) as usize;
        let pos = self.rope.line_to_char(target) + col.min(self.line_len(target));
        self.move_to(pos, select);
    }

    /// 줄 처음/끝으로.
    pub(crate) fn home(&mut self, select: bool) {
        let (line, _) = self.cursor_line_col();
        self.move_to(self.rope.line_to_char(line), select);
    }

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

    /// 단어 삭제(Ctrl+Backspace=왼쪽 / Ctrl+Delete=오른쪽). 선택이 있으면 선택 삭제.
    pub(crate) fn delete_word(&mut self, right: bool) {
        if self.selection().is_some() {
            self.backspace();
            return;
        }
        let target = if right { self.word_right() } else { self.word_left() };
        let c = self.cursor();
        if target == c {
            return;
        }
        self.begin(EditKind::Delete, true); // 단어 삭제는 별도 묶음.
        let (a, b) = (c.min(target), c.max(target));
        self.rope.remove(a..b);
        self.set_cursor(a);
        self.dirty = true;
        self.ensure_visible = true;
    }

    pub(crate) fn undo(&mut self) {
        if let Some((r, c)) = self.undo.pop() {
            let snap = (self.rope.clone(), self.cursor());
            self.redo.push(snap);
            self.rope = r;
            self.set_cursor(c.min(self.rope.len_chars()));
            self.dirty = true;
            self.ensure_visible = true;
        }
        self.undo_open = false;
        self.last_kind = None;
    }

    pub(crate) fn redo(&mut self) {
        if let Some((r, c)) = self.redo.pop() {
            let snap = (self.rope.clone(), self.cursor());
            self.undo.push(snap);
            self.rope = r;
            self.set_cursor(c.min(self.rope.len_chars()));
            self.dirty = true;
            self.ensure_visible = true;
        }
        self.undo_open = false;
        self.last_kind = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::editbuf::EditBuf;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "LF")
    }

    #[test]
    fn insert_and_backspace() {
        let mut b = buf("");
        b.insert("hello");
        assert_eq!(b.rope.to_string(), "hello");
        assert_eq!(b.cursor(), 5);
        b.backspace();
        assert_eq!(b.rope.to_string(), "hell");
    }

    #[test]
    fn selection_replace() {
        let mut b = buf("abcdef");
        b.sel = crate::editsel::Selection::single(4, 1); // "bcd"
        b.insert("X");
        assert_eq!(b.rope.to_string(), "aXef");
        assert_eq!(b.cursor(), 2);
        assert!(b.selection().is_none());
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
    fn undo_coalesces_consecutive_typing() {
        let mut b = buf("a");
        b.set_cursor(1);
        b.insert("b");
        b.insert("c"); // 연속 → 한 묶음
        assert_eq!(b.rope.to_string(), "abc");
        b.undo();
        assert_eq!(b.rope.to_string(), "a"); // 묶음 전체 취소
        b.redo();
        assert_eq!(b.rope.to_string(), "abc");
    }

    #[test]
    fn cursor_move_breaks_undo_group() {
        let mut b = buf("");
        b.insert("ab");
        b.move_to(0, false); // 경계
        b.insert("X"); // 새 묶음
        assert_eq!(b.rope.to_string(), "Xab");
        b.undo();
        assert_eq!(b.rope.to_string(), "ab");
        b.undo();
        assert_eq!(b.rope.to_string(), "");
    }

    #[test]
    fn word_movement_and_delete() {
        let mut b = buf("foo bar_baz qux");
        b.set_cursor(0);
        assert_eq!(b.word_right(), 4); // "foo " → "bar_baz" 시작
        b.set_cursor(12);
        assert_eq!(b.word_left(), 4); // 비단어+단어 역순 → bar_baz 시작
        b.set_cursor(0);
        b.delete_word(true); // "foo " 삭제
        assert_eq!(b.rope.to_string(), "bar_baz qux");
        assert_eq!(b.cursor(), 0);
    }

    #[test]
    fn insert_then_delete_are_separate_groups() {
        let mut b = buf("");
        b.insert("abc");
        b.backspace(); // 종류 전환 → 새 묶음
        assert_eq!(b.rope.to_string(), "ab");
        b.undo();
        assert_eq!(b.rope.to_string(), "abc");
        b.undo();
        assert_eq!(b.rope.to_string(), "");
    }

    #[test]
    fn newline_keeps_indentation() {
        let mut b = buf("    ab");
        b.set_cursor(6); // 줄 끝
        b.insert_newline();
        assert_eq!(b.rope.to_string(), "    ab\n    "); // 선행 공백 복사
        assert_eq!(b.cursor(), 11);
    }
}
