//! 대용량 편집기의 **줄 단위 편집** — 줄 복제·줄 삭제.
//!
//! 작은 문서 쪽 우클릭 메뉴에는 처음부터 있었는데(`ctx.dupline`·`ctx.delline`) 대용량
//! 편집기에는 없었다(2026-09-07 쌍둥이 비교). 큰 로그·설정 파일을 손볼 때야말로 줄을
//! 복제해 고치는 일이 잦은데, 정작 그 창에서만 없었다.
//!
//! ## 왜 마지막 줄이 함정인가
//!
//! 대부분의 줄은 줄바꿈으로 끝나므로 "줄 시작부터 다음 줄 시작까지"가 곧 그 줄이다.
//! 그런데 **파일 끝의 줄에는 줄바꿈이 없다.** 그대로 같은 규칙을 쓰면
//!
//! * 복제할 때 — 줄 시작에 글만 넣어 `abcabc` 처럼 한 줄이 되고,
//! * 지울 때 — 앞 줄의 줄바꿈이 남아 **빈 줄이 하나 생긴다.**
//!
//! 그래서 아래 두 함수가 `has_nl`(그 줄이 줄바꿈으로 끝나는가)을 받는다. 눈으로는 잘
//! 안 보이는 차이라 시험으로 못 박아 둔다.

/// 복제한 줄을 **넣을 자리**(char 색인).
///
/// 줄바꿈으로 끝나면 줄 시작에 넣으면 되고(그 줄이 통째로 아래로 밀린다), 파일 끝 줄이면
/// 문서 끝에 붙인다.
pub fn dup_at(start: usize, end: usize, has_nl: bool) -> usize {
    match has_nl {
        true => start,
        false => end,
    }
}

/// 복제할 때 **넣을 글**(`line` 은 줄바꿈을 뗀 알맹이).
///
/// 줄바꿈은 어느 쪽이든 **반드시 하나 붙는다** — 붙이는 자리만 다르다. 보통 줄은 뒤에
/// 붙여 그 줄이 아래로 밀리게 하고, 파일 끝 줄은 앞에 붙여 새 줄이 만들어지게 한다.
/// (처음엔 보통 줄에 줄바꿈을 안 붙였다가 `aaa/bbbbbb/ccc` 처럼 한 줄로 붙어 버렸다 —
/// 아래 왕복 시험이 잡았다.)
pub fn dup_text(line: &str, has_nl: bool) -> String {
    match has_nl {
        true => format!("{line}\n"),
        false => format!("\n{line}"),
    }
}

/// 줄을 지울 **char 범위** `[a, b)`.
///
/// 파일 끝 줄이면 앞 줄의 줄바꿈까지 함께 지운다 — 안 그러면 빈 줄이 남는다.
/// 다만 문서에 줄이 하나뿐이면 지울 앞 줄바꿈이 없다.
pub fn del_range(start: usize, end: usize, has_nl: bool, has_prev: bool) -> (usize, usize) {
    match (has_nl, has_prev) {
        (true, _) => (start, end),
        (false, true) => (start.saturating_sub(1), end),
        (false, false) => (start, end),
    }
}

impl crate::editbuf::EditBuf {
    /// 그 줄이 차지하는 char 범위와 줄바꿈 여부 — `(시작, 끝, 줄바꿈으로 끝나는가)`.
    fn line_span(&self, line: usize) -> Option<(usize, usize, bool)> {
        if line >= self.rope.len_lines() {
            return None;
        }
        let start = self.rope.line_to_char(line);
        let end = match line + 1 < self.rope.len_lines() {
            true => self.rope.line_to_char(line + 1),
            false => self.rope.len_chars(),
        };
        let has_nl = end > start && self.rope.char(end - 1) == '\n';
        Some((start, end, has_nl))
    }

    /// 커서가 있는 줄을 바로 아래에 한 벌 더 만든다.
    ///
    /// 선택은 건드리지 않는다 — 선택이 남아 있으면 `insert` 가 그것을 대체해 버리므로
    /// 먼저 캐럿으로 접는다.
    pub fn dup_line(&mut self) {
        let line = self.cursor_line_col().0;
        let Some((start, end, has_nl)) = self.line_span(line) else { return };
        let text: String = self.rope.slice(start..end).chars().collect();
        let text = text.trim_end_matches('\n').to_string();
        let at = dup_at(start, end, has_nl);
        let ins = dup_text(&text, has_nl);
        self.set_cursor(at);
        self.insert(&ins);
    }

    /// 커서가 있는 줄을 지운다.
    ///
    /// 범위를 **선택으로 만들어 한 번에** 지운다. 한 글자씩 지우면 되돌리기가 글자 수만큼
    /// 쌓여, 지운 줄 하나를 되돌리려고 Ctrl+Z 를 수십 번 눌러야 한다.
    pub fn del_line(&mut self) {
        let line = self.cursor_line_col().0;
        let Some((start, end, has_nl)) = self.line_span(line) else { return };
        let (a, b) = del_range(start, end, has_nl, line > 0);
        if a >= b {
            return;
        }
        self.set_cursor(a);
        self.move_head(b);
        self.delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 보통 줄은 줄 시작에 그대로 넣는다.
    #[test]
    fn 보통_줄은_줄_시작에_넣는다() {
        assert_eq!(dup_at(10, 14, true), 10);
        assert_eq!(dup_text("abc", true), "abc\n");
    }

    /// 파일 끝 줄은 문서 끝에 **줄바꿈을 붙여** 넣는다 — 안 그러면 한 줄이 된다.
    #[test]
    fn 파일_끝_줄은_줄바꿈을_붙인다() {
        assert_eq!(dup_at(10, 13, false), 13);
        assert_eq!(dup_text("abc", false), "\nabc");
    }

    /// 보통 줄은 줄바꿈까지 통째로 지운다.
    #[test]
    fn 보통_줄은_줄바꿈까지_지운다() {
        assert_eq!(del_range(10, 14, true, true), (10, 14));
        assert_eq!(del_range(0, 4, true, false), (0, 4));
    }

    /// 파일 끝 줄은 **앞 줄바꿈까지** 지워야 빈 줄이 남지 않는다.
    #[test]
    fn 파일_끝_줄은_앞_줄바꿈까지_지운다() {
        assert_eq!(del_range(10, 13, false, true), (9, 13));
        // 줄이 하나뿐이면 지울 앞 줄바꿈이 없다(0 아래로 내려가면 안 된다).
        assert_eq!(del_range(0, 3, false, false), (0, 3));
    }

    /// 실제 버퍼에서 오갔다 와도 글이 맞아야 한다.
    #[test]
    fn 복제하고_지우면_제자리로_온다() {
        for (text, line) in [("aaa\nbbb\nccc", 1usize), ("aaa\nbbb\nccc", 2), ("only", 0)] {
            let mut eb = crate::editbuf::EditBuf::new_buf(text, "UTF-8".into(), "LF");
            let at = eb.rope.line_to_char(line);
            eb.set_cursor(at);
            eb.dup_line();
            let after_dup: String = eb.rope.chars().collect();
            assert_eq!(after_dup.lines().count(), text.lines().count() + 1, "{text:?} 줄 {line}");
            // 복제된 줄로 커서를 옮겨 지우면 원래대로 돌아와야 한다.
            eb.set_cursor(eb.rope.line_to_char(line));
            eb.del_line();
            let back: String = eb.rope.chars().collect();
            assert_eq!(back, text, "{text:?} 줄 {line} 을 복제했다 지웠는데 달라졌다");
        }
    }

    /// 지운 줄은 되돌리기 **한 번**에 돌아와야 한다(글자마다 쌓이면 안 된다).
    #[test]
    fn 줄_삭제는_되돌리기_한_번() {
        let mut eb = crate::editbuf::EditBuf::new_buf("aaa\nbbbbbbbbbb\nccc", "UTF-8".into(), "LF");
        eb.set_cursor(eb.rope.line_to_char(1));
        eb.del_line();
        assert_eq!(eb.rope.chars().collect::<String>(), "aaa\nccc");
        eb.undo();
        assert_eq!(eb.rope.chars().collect::<String>(), "aaa\nbbbbbbbbbb\nccc");
    }
}
