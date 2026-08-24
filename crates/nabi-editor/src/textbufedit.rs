//! [`TextBuf`]의 **편집·되돌리기** — 넣기·지우기와 undo/redo. 이동·선택은 textbuf.
//!
//! 모든 편집이 [`TextBuf::edit`] 한 곳을 지난다. 되돌리기 칸을 쌓는 자리가 하나뿐이라야
//! "어떤 편집은 되돌려지지 않는다" 같은 구멍이 안 생긴다.

use crate::textbuf::{step_pos, Delta, TextBuf};

/// 되돌리기 스택이 들고 있을 수 있는 최대 바이트.
///
/// 되돌리기 칸은 지워진 바이트를 통째로 들고 있다. 1GB를 선택해 지우면 그 1GB가 스택에
/// 얹히고, 되돌리기·다시 실행을 반복하면 덧댐 버퍼에도 매번 다시 쌓인다(교차 검토
/// 2026-08-25). 상한을 넘으면 **가장 오래된 칸부터** 버린다 — 최근 편집을 되돌리는 것이
/// 오래전 것을 되돌리는 것보다 훨씬 자주 쓰인다. HEX 편집기(edithex)와 같은 방침이다.
const MAX_UNDO_BYTES: usize = 64_000_000;

impl TextBuf {

    /// `[at, at+del)`을 `ins`로 바꾸고 되돌리기 칸을 쌓는다. 모든 편집이 여기를 지난다.
    fn edit(&mut self, at: u64, del: u64, ins: &[u8], caret_after: u64) {
        if self.readonly {
            return;
        }
        let removed = self.data.read(at, del as usize);
        let cont = self.group;
        self.undo.push(Delta { at, removed, ins_len: ins.len() as u64, caret: self.caret, anchor: self.anchor, cont });
        self.trim_undo();
        self.redo.clear();
        self.data.splice(at, del, ins);
        self.caret = caret_after;
        self.anchor = caret_after;
        self.dirty = true;
    }

    /// 선택이 있으면 지운다. 지웠으면 참.
    fn drop_selection(&mut self) -> bool {
        let (a, b) = self.selection();
        if a == b {
            return false;
        }
        self.edit(a, b - a, &[], a);
        true
    }

    /// 글자를 넣는다(선택이 있으면 대신 들어간다). 연속 입력은 한 되돌리기 칸으로 묶인다.
    pub fn insert(&mut self, s: &str) -> bool {
        if self.readonly {
            return false;
        }
        let had = self.drop_selection();
        if had {
            self.group = true; // 선택 삭제와 삽입은 한 동작이다.
        }
        // 이 인코딩으로 적을 수 없는 글자면 넣지 않는다 — 조용히 `&#128512;` 같은 것이
        // 박히느니 아무 일도 일어나지 않는 편이 낫다(호출부가 사용자에게 알린다).
        let Some(bytes) = self.data.encode(s) else { return false };
        let at = self.caret;
        self.edit(at, 0, &bytes, at + bytes.len() as u64);
        self.group = true;
        self.goal_col = None;
        true
    }

    /// 줄바꿈을 넣는다 — 문서의 원래 EOL로.
    pub fn insert_newline(&mut self) {
        if self.readonly {
            return;
        }
        let had = self.drop_selection();
        if had {
            self.group = true;
        }
        let nl = self.data.eol_bytes().to_vec();
        let at = self.caret;
        self.edit(at, 0, &nl, at + nl.len() as u64);
        self.group = true;
        self.goal_col = None;
    }

    /// 백스페이스(왼쪽 한 글자) 또는 Delete(오른쪽 한 글자).
    ///
    /// 지우는 범위를 커서 이동과 **같은 걸음**으로 정한다. 그래야 CRLF가 반만 남거나
    /// CP949 두 바이트 중 하나만 지워지는 일이 없다 — 한 곳에서만 경계를 정하기 때문이다.
    pub fn erase(&mut self, forward: bool) {
        if self.readonly || self.drop_selection() {
            self.group = true;
            return;
        }
        let other = step_pos(&self.data, self.caret, forward);
        let (at, end) = if forward { (self.caret, other) } else { (other, self.caret) };
        if at >= end {
            return;
        }
        self.edit(at, end - at, &[], at);
        self.group = true;
        self.goal_col = None;
    }

    /// 되돌리기 — 묶인 칸은 한 번에 전부.
    ///
    /// 되돌리기와 다시 실행은 **같은 모양의 칸**이다: 둘 다 "그 자리의 지금 바이트를 빼고,
    /// 들고 있던 바이트를 넣는다". 그래서 되돌리기 직전에 지금 바이트를 챙겨 두면 그것이
    /// 그대로 다시 실행 칸이 된다(챙기지 않으면 되돌린 편집을 되살릴 방법이 없다).
    pub fn undo(&mut self) {
        if self.readonly || self.undo.is_empty() {
            return; // 읽기 전용이면 되돌리기도 편집이다. 이력이 없으면 수정 표시도 건드리지 않는다.
        }
        while let Some(d) = self.undo.pop() {
            let flip = self.flip(&d);
            self.redo.push(flip);
            if !d.cont {
                break; // 앞 칸과 묶여 있었다면 그 칸까지 이어서 되돌린다.
            }
        }
        self.dirty = true;
        self.group = false;
    }

    /// 칸 하나를 적용하고, 그 반대 방향 칸을 만들어 돌려준다.
    fn flip(&mut self, d: &Delta) -> Delta {
        let now = self.data.read(d.at, d.ins_len as usize); // 지금 그 자리에 있는 바이트.
        self.data.splice(d.at, d.ins_len, &d.removed);
        let total = self.data.total();
        self.caret = d.caret.min(total);
        self.anchor = d.anchor.min(total);
        Delta {
            at: d.at, removed: now, ins_len: d.removed.len() as u64,
            caret: d.caret, anchor: d.anchor, cont: d.cont,
        }
    }

    /// 다시 실행 — 되돌리기와 대칭으로, 묶인 칸은 한 번에 전부.
    pub fn redo(&mut self) {
        if self.readonly || self.redo.is_empty() {
            return;
        }
        while let Some(d) = self.redo.pop() {
            let flip = self.flip(&d);
            self.caret = (flip.at + flip.ins_len).min(self.data.total());
            self.anchor = self.caret;
            self.undo.push(flip);
            // 다음 칸도 같은 묶음이면 이어서 실행한다(되돌리기가 쌓은 순서를 거슬러 올라간다).
            if !self.redo.last().is_some_and(|n| n.cont) {
                break;
            }
        }
        self.dirty = true;
        self.group = false;
    }

    /// 되돌리기 스택이 [`MAX_UNDO_BYTES`]를 넘으면 오래된 칸부터 버린다.
    ///
    /// 묶인 칸(`cont`)의 한가운데를 자르면 되돌리기가 반만 적용된다. 그래서 묶음의 **첫 칸**
    /// (`cont == false`)까지 함께 버려 항상 온전한 묶음 단위로만 잘라 낸다.
    fn trim_undo(&mut self) {
        let mut total: usize = self.undo.iter().map(|d| d.removed.len()).sum();
        while total > MAX_UNDO_BYTES && self.undo.len() > 1 {
            let d = self.undo.remove(0);
            total -= d.removed.len();
            // 방금 버린 칸에 이어 붙어 있던 나머지도 같이 버린다 — 반쪽 묶음은 위험하다.
            while self.undo.first().is_some_and(|n| n.cont) {
                total -= self.undo.remove(0).removed.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::textbuf::TextBuf;
    use crate::textdata::TextData;

    fn buf(s: &str) -> TextBuf {
        TextBuf::new(TextData::from_vec(s.as_bytes().to_vec()))
    }

    fn text(b: &TextBuf) -> String {
        b.data.to_string_lossy()
    }

#[test]
    fn typing_inserts_at_the_caret() {
        let mut b = buf("ac");
        b.go(1, false);
        b.insert("b");
        assert_eq!(text(&b), "abc");
        assert_eq!(b.caret, 2);
    }

    #[test]
    fn backspace_removes_the_character_before_the_caret() {
        let mut b = buf("abc");
        b.go(2, false);
        b.erase(false);
        assert_eq!(text(&b), "ac");
        assert_eq!(b.caret, 1);
    }

    #[test]
    fn delete_removes_the_character_after_the_caret() {
        let mut b = buf("abc");
        b.go(1, false);
        b.erase(true);
        assert_eq!(text(&b), "ac");
        assert_eq!(b.caret, 1);
    }

    /// 한글은 UTF-8에서 3바이트다 — 한 번 눌러 한 글자가 지워져야 한다.
    #[test]
    fn one_backspace_removes_one_hangul_character() {
        let mut b = buf("가나");
        b.go(6, false);
        b.erase(false);
        assert_eq!(text(&b), "가");
        assert_eq!(b.caret, 3);
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut b = buf("hello world");
        b.go(0, false);
        b.go(5, true);
        b.insert("bye");
        assert_eq!(text(&b), "bye world");
    }

    #[test]
    fn enter_uses_the_documents_own_line_ending() {
        let mut b = buf("a\r\nb");
        b.go(4, false); // "b" 뒤.
        b.insert_newline();
        assert_eq!(text(&b), "a\r\nb\r\n");
        assert_eq!(b.data.lines(), 3);
    }

    /// CRLF 앞에서 백스페이스 한 번이면 줄 끝이 통째로 지워져야 한다 — CR만 남으면 안 된다.
    #[test]
    fn backspace_over_a_crlf_removes_both_bytes() {
        let mut b = buf("a\r\nb");
        b.go(3, false); // "\r\n" 뒤.
        b.erase(false);
        assert_eq!(text(&b), "ab");
        assert_eq!(b.data.lines(), 1);
    }

    /// 이어 친 글자는 되돌리기 **한 번**에 전부 사라져야 한다.
    #[test]
    fn a_run_of_typing_undoes_as_one_step() {
        let mut b = buf("");
        for c in "hello".chars() {
            b.insert(&c.to_string());
        }
        assert_eq!(text(&b), "hello");
        b.undo();
        assert_eq!(text(&b), "");
        assert!(!b.can_undo());
    }

    /// 커서를 옮기고 다시 치면 별개의 묶음이어야 한다.
    #[test]
    fn moving_the_caret_starts_a_new_undo_step() {
        let mut b = buf("xy");
        b.go(0, false);
        b.insert("A");
        b.go(3, false);
        b.insert("B");
        assert_eq!(text(&b), "AxyB");
        b.undo();
        assert_eq!(text(&b), "Axy");
        b.undo();
        assert_eq!(text(&b), "xy");
    }

    /// 되돌리기와 다시 실행을 여러 번 오가도 같은 두 상태만 왔다 갔다 해야 한다.
    #[test]
    fn undo_and_redo_can_be_cycled_without_drifting() {
        let mut b = buf("one\ntwo");
        b.go(3, false);
        b.insert(" and a half");
        let after = text(&b);
        for _ in 0..4 {
            b.undo();
            assert_eq!(text(&b), "one\ntwo");
            b.redo();
            assert_eq!(text(&b), after);
        }
        assert_eq!(b.data.lines(), 2);
    }

    #[test]
    fn redo_puts_back_what_undo_took_away() {
        let mut b = buf("start");
        b.go(5, false);
        b.insert("!");
        b.undo();
        assert_eq!(text(&b), "start");
        b.redo();
        assert_eq!(text(&b), "start!");
    }

    #[test]
    fn a_readonly_buffer_refuses_every_edit() {
        let mut b = buf("keep");
        b.readonly = true;
        b.go(2, false);
        b.insert("X");
        b.erase(false);
        b.insert_newline();
        assert_eq!(text(&b), "keep");
        assert!(!b.dirty);
    }

    /// 그리고 그 문서에서 Delete 한 번은 한 글자만 지운다.
    #[test]
    fn one_delete_removes_exactly_one_cp949_character() {
        let d = crate::textdata::TextData::from_vec(vec![0xB0, 0xA1, 0xB3, 0xAA]);
        let mut b = TextBuf::new(d);
        b.erase(true);
        assert_eq!(b.data.total(), 2, "두 바이트만 지워져야 한다");
        assert_eq!(b.data.line(0), "나");
    }

    /// 이 인코딩으로 못 적는 글자는 넣지 않는다 — `&#128512;`가 박히면 내용이 달라진다.
    #[test]
    fn a_character_the_encoding_cannot_hold_is_refused() {
        let d = crate::textdata::TextData::from_vec(vec![0xB0, 0xA1]); // CP949로 감지.
        let mut b = TextBuf::new(d);
        let before = b.data.total();
        assert!(!b.insert("\u{1f600}"), "넣을 수 없으면 false를 돌려줘야 한다");
        assert_eq!(b.data.total(), before, "문서가 바뀌면 안 된다");
        assert!(!b.dirty);
    }

    /// 읽기 전용이면 되돌리기·다시 실행도 막아야 한다 — 그것도 문서를 바꾸는 일이다.
    #[test]
    fn a_readonly_buffer_refuses_undo_and_redo_too() {
        let mut b = buf("a");
        b.go(1, false);
        b.insert("b");
        b.readonly = true;
        b.undo();
        assert_eq!(text(&b), "ab", "읽기 전용인데 되돌려졌다");
        b.readonly = false;
        b.undo();
        b.readonly = true;
        b.redo();
        assert_eq!(text(&b), "a", "읽기 전용인데 다시 실행됐다");
    }

    /// 이력이 비었으면 되돌리기를 눌러도 '수정됨'이 되면 안 된다.
    #[test]
    fn undo_on_an_untouched_buffer_does_not_mark_it_dirty() {
        let mut b = buf("x");
        b.undo();
        b.redo();
        assert!(!b.dirty);
    }

    /// 선택을 바꿔 친 것을 되돌리면 그 선택이 되살아나야 한다.
    #[test]
    fn undo_restores_the_selection_that_was_replaced() {
        let mut b = buf("abc");
        b.go(1, false);
        b.go(2, true); // "b" 선택.
        b.insert("X");
        b.undo();
        assert_eq!(text(&b), "abc");
        assert_eq!(b.selection(), (1, 2), "선택이 그대로 돌아와야 한다");
    }

    /// 큰 삭제를 반복해도 되돌리기 스택이 무한정 커지면 안 된다.
    #[test]
    fn the_undo_stack_has_a_ceiling() {
        let big = "x".repeat(200_000);
        let mut b = buf(&big.repeat(2));
        for _ in 0..800 {
            b.go(0, false);
            b.go(100_000, true);
            b.erase(false); // 10만 바이트씩 지운다 — 되돌리기 칸이 그만큼 쌓인다.
            b.undo();
        }
        let held: usize = b.undo_bytes();
        assert!(held <= 64_000_000, "되돌리기가 {held}바이트나 들고 있다");
    }

    #[test]
    fn deleting_a_selection_that_spans_lines_joins_them() {
        let mut b = buf("one\ntwo\nthree");
        b.go(2, false);
        b.go(9, true);
        b.erase(false);
        assert_eq!(text(&b), "onhree");
        assert_eq!(b.data.lines(), 1);
    }
}
