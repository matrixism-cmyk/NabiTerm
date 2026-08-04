//! 편집 버퍼(E6) 변경 연산 + undo/redo(연속 편집 묶음 coalescing). editbuf.rs의 EditBuf impl.
//!
//! 같은 종류(삽입/삭제) 연속 편집은 한 undo 단위로 묶어 Ctrl+Z가 글자별이 아니라 구간별로 동작한다.
//! 커서 이동·종류 전환·선택 대체·줄바꿈은 묶음 경계.

use crate::editbuf::{EditBuf, EditKind};

/// undo 스냅샷 최대 개수(rope clone은 구조 공유라 메모리 부담은 작음).
const UNDO_MAX: usize = 1000;
/// 연속 타자로 볼 최대 간격(ms). CodeMirror 기본값과 같다.
const GROUP_MS: u128 = 500;

impl EditBuf {
    /// 편집 직전 호출 — 같은 종류 연속이면 묶고(스냅샷 생략), 아니면 새 스냅샷을 쌓는다.
    ///
    /// 묶는 조건은 **시간 AND 인접성**이다. 시간만 보면 취소 단위가 타자 속도에 따라
    /// 들쭉날쭉해지고, 인접성만 보면 한참 뒤에 같은 자리를 고쳐도 예전 편집과 한 덩어리가 된다.
    fn begin(&mut self, kind: EditKind, fresh: bool) {
        let recent = self.last_time.map(|t| t.elapsed().as_millis() <= GROUP_MS).unwrap_or(false);
        let adjacent = self.cursor() == self.last_at;
        if !fresh && self.undo_open && self.last_kind == Some(kind) && recent && adjacent {
            self.redo.clear(); // 묶음 계속 — 스냅샷 추가 안 함.
        } else {
            let snap = (self.rope.clone(), self.cursor());
            self.undo.push(snap);
            if self.undo.len() > UNDO_MAX {
                self.undo.remove(0);
                // 가장 오래된 스냅샷을 버리면 저장 지점의 깊이도 한 칸 당겨진다.
                self.saved_depth = self.saved_depth.and_then(|d| d.checked_sub(1));
            }
            self.redo.clear();
            self.undo_open = true;
        }
        self.last_kind = Some(kind);
    }

    /// 편집 후 공통 마무리 — 묶음 판정 기준 갱신 + 수정 표시 동기화 + 커서 따라가기.
    fn after_edit(&mut self) {
        self.last_at = self.cursor();
        self.last_time = Some(std::time::Instant::now());
        self.ensure_visible = true;
        self.sync_dirty();
    }

    /// 마지막 저장 시점과 같은 깊이면 수정 표시를 지운다(되돌려 원래대로 온 경우).
    pub(crate) fn sync_dirty(&mut self) {
        self.dirty = self.saved_depth != Some(self.undo.len());
    }

    /// 저장 완료 표시. 이후 되돌려 이 상태로 오면 수정 표시가 다시 사라진다.
    pub(crate) fn mark_saved(&mut self) {
        self.saved_depth = Some(self.undo.len());
        self.undo_open = false; // 저장 뒤 타자는 새 묶음 — 한 번의 취소가 저장 지점을 넘지 않게.
        self.dirty = false;
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
        self.after_edit();
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
            let (c, a) = (self.cursor(), self.step_left()); // grapheme 통째로 지운다.
            self.rope.remove(a..c);
            self.set_cursor(a);
        }
        self.after_edit();
    }

    /// Delete(앞 글자 삭제, 선택 우선).
    pub(crate) fn delete(&mut self) {
        let fresh = self.selection().is_some();
        self.begin(EditKind::Delete, fresh);
        if self.selection().is_some() {
            self.del_selection();
        } else if self.cursor() < self.rope.len_chars() {
            let (c, b) = (self.cursor(), self.step_right()); // grapheme 통째로.
            self.rope.remove(c..b);
        }
        self.after_edit();
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
        self.after_edit();
    }

    pub(crate) fn undo(&mut self) {
        if let Some((r, c)) = self.undo.pop() {
            let snap = (self.rope.clone(), self.cursor());
            self.redo.push(snap);
            self.rope = r;
            self.set_cursor(c.min(self.rope.len_chars()));
            self.ensure_visible = true;
            self.sync_dirty(); // 저장 지점까지 되돌아왔으면 수정 표시가 사라진다.
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
            self.ensure_visible = true;
            self.sync_dirty();
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
    fn backspace_deletes_whole_grapheme() {
        // e + 결합 악센트는 char 2개 — 한 번의 백스페이스로 통째로 지워야 한다.
        let mut b = buf("xe\u{0301}");
        b.set_cursor(3);
        b.backspace();
        assert_eq!(b.rope.to_string(), "x");
        assert_eq!(b.cursor(), 1);
    }




    #[test]
    fn distant_edits_are_separate_undo_groups() {
        // 멀리 떨어진 자리를 고치면 붙어 있던 타자와 한 덩어리가 되면 안 된다.
        let mut b = buf("abcdefghij");
        b.set_cursor(1);
        b.insert("X");
        b.set_cursor(8); // 이동은 묶음 경계지만, 인접성 조건이 이중으로 막아준다.
        b.insert("Y");
        b.undo();
        assert_eq!(b.rope.to_string(), "aXbcdefghij", "마지막 편집만 취소");
        b.undo();
        assert_eq!(b.rope.to_string(), "abcdefghij");
    }

    #[test]
    fn returning_to_saved_state_clears_dirty() {
        // 저장 후 고쳤다가 되돌리면 수정 표시가 사라져야 한다(VS Code 동작).
        let mut b = buf("abc");
        b.mark_saved();
        assert!(!b.dirty);
        b.set_cursor(3);
        b.insert("d");
        assert!(b.dirty, "고쳤으니 수정 표시");
        b.undo();
        assert!(!b.dirty, "저장 지점으로 돌아왔으니 수정 표시 해제");
        b.redo();
        assert!(b.dirty);
    }

    #[test]
    fn save_closes_undo_group() {
        // 저장 직후의 타자가 저장 전 타자와 묶이면, 한 번의 취소가 저장 지점을 건너뛴다.
        let mut b = buf("");
        b.insert("a");
        b.mark_saved();
        b.insert("b");
        b.undo();
        assert_eq!(b.rope.to_string(), "a", "저장 시점까지만 취소");
        assert!(!b.dirty);
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
