//! 대용량 편집기의 **줄 단위 편집** — 줄 복제·삭제·위아래 이동.
//!
//! 작은 문서 쪽 우클릭 메뉴에는 처음부터 있었는데(`ctx.dupline`·`ctx.delline`·
//! `ctx.moveup`·`ctx.movedown`) 대용량 편집기에는 없었다(2026-09-07 쌍둥이 비교).
//! 큰 로그·설정 파일을 손볼 때야말로 줄을 복제해 고치는 일이 잦은데, 정작 그 창에서만
//! 없었다. Ctrl+D 도 여기서는 "다음 일치 선택"이라 대신 쓸 수도 없었다.
//!
//! ## 왜 마지막 줄이 함정인가
//!
//! 대부분의 줄은 줄바꿈으로 끝나므로 "줄 시작부터 다음 줄 시작까지"가 곧 그 줄이다.
//! 그런데 **파일 끝의 줄에는 줄바꿈이 없다.** 그대로 같은 규칙을 쓰면
//!
//! * 복제할 때 — 줄 시작에 글만 넣어 `abcabc` 처럼 한 줄이 되고,
//! * 지울 때 — 앞 줄의 줄바꿈이 남아 **빈 줄이 하나 생긴다.**
//! * 맞바꿀 때 — 줄바꿈이 하나 늘어 **파일 끝에 빈 줄이 생긴다.**
//!
//! 그래서 아래 함수들이 `has_nl`(그 줄이 줄바꿈으로 끝나는가)을 받는다. 눈으로는 잘
//! 안 보이는 차이라 시험으로 못 박아 둔다.
//!
//! ## 되돌리기는 한 번이어야 한다
//!
//! 셋 다 **한 번의 편집**으로 끝낸다(범위를 선택해 두고 새 글로 대체). 지웠다 넣으면
//! 되돌리기가 두 번 쌓이고, 한 글자씩 지우면 글자 수만큼 쌓인다 — 줄 하나 되돌리려고
//! Ctrl+Z 를 수십 번 눌러야 하는 것은 되돌리기가 아니다.

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

/// 두 줄을 맞바꿨을 때 그 자리에 들어갈 글.
///
/// `a` 가 위, `b` 가 아래이고 둘 다 줄바꿈을 뗀 알맹이다. `tail_nl` 은 **아래 줄이
/// 줄바꿈으로 끝나는가** — 파일 끝이면 거짓이고, 그때는 바뀐 뒤에도 끝에 줄바꿈이
/// 없어야 한다. 있던 줄바꿈이 하나 늘면 파일 끝에 빈 줄이 생긴다.
pub fn swap_text(a: &str, b: &str, tail_nl: bool) -> String {
    match tail_nl {
        true => format!("{b}\n{a}\n"),
        false => format!("{b}\n{a}"),
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

    /// 그 줄의 알맹이(줄바꿈을 뗀 글).
    fn line_body(&self, a: usize, b: usize) -> String {
        self.rope.slice(a..b).chars().collect::<String>().trim_end_matches('\n').to_string()
    }

    /// 커서가 있는 줄을 바로 아래에 한 벌 더 만든다.
    pub fn dup_line(&mut self) {
        let line = self.cursor_line_col().0;
        let Some((start, end, has_nl)) = self.line_span(line) else { return };
        let text = self.line_body(start, end);
        let at = dup_at(start, end, has_nl);
        let ins = dup_text(&text, has_nl);
        // 선택이 남아 있으면 `insert` 가 그것을 대체해 버린다 — 먼저 캐럿으로 접는다.
        self.set_cursor(at);
        self.insert(&ins);
    }

    /// 커서가 있는 줄을 지운다.
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

    /// 커서가 있는 줄을 위나 아래로 한 칸 옮긴다.
    pub fn move_line(&mut self, up: bool) {
        let cur = self.cursor_line_col().0;
        // "이 줄을 아래로"는 "아래 줄을 위로"와 같은 일이다 — 규칙을 한 벌만 둔다.
        let i = match up {
            true => cur,
            false => cur + 1,
        };
        if i == 0 || i >= self.rope.len_lines() {
            return;
        }
        let (Some((pstart, pend, _)), Some((start, end, has_nl))) =
            (self.line_span(i - 1), self.line_span(i))
        else {
            return;
        };
        // 글이 줄바꿈으로 끝나면 ropey 는 맨 뒤에 빈 줄을 하나 더 센다. 그 빈 줄과
        // 맞바꾸면 줄이 사라진 것처럼 보인다 — 옮길 것이 없으면 아무것도 하지 않는다.
        if start >= end && !has_nl {
            return;
        }
        let (above, below) = (self.line_body(pstart, pend), self.line_body(start, end));
        let moved = swap_text(&above, &below, has_nl);
        self.set_cursor(pstart);
        self.move_head(end);
        self.insert(&moved);
        // 커서는 **옮긴 줄을 따라간다** — 제자리에 두면 한 번 더 눌렀을 때 엉뚱한 줄이 간다.
        let back = match up {
            true => pstart,
            false => pstart + below.chars().count() + 1,
        };
        self.set_cursor(back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> crate::editbuf::EditBuf {
        crate::editbuf::EditBuf::new_buf(text, "UTF-8".into(), "LF")
    }

    fn text_of(eb: &crate::editbuf::EditBuf) -> String {
        eb.rope.chars().collect()
    }

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

    /// 맞바꾼 글은 줄바꿈 개수가 그대로여야 한다 — 하나 늘면 파일 끝에 빈 줄이 생긴다.
    #[test]
    fn 맞바꿔도_줄바꿈이_안_는다() {
        assert_eq!(swap_text("a", "b", true), "b\na\n");
        assert_eq!(swap_text("a", "b", false), "b\na");
    }

    /// 실제 버퍼에서 오갔다 와도 글이 맞아야 한다.
    #[test]
    fn 복제하고_지우면_제자리로_온다() {
        for (text, line) in [("aaa\nbbb\nccc", 1usize), ("aaa\nbbb\nccc", 2), ("only", 0)] {
            let mut eb = buf(text);
            eb.set_cursor(eb.rope.line_to_char(line));
            eb.dup_line();
            assert_eq!(
                text_of(&eb).lines().count(),
                text.lines().count() + 1,
                "{text:?} 줄 {line}"
            );
            // 복제된 줄로 커서를 옮겨 지우면 원래대로 돌아와야 한다.
            eb.set_cursor(eb.rope.line_to_char(line));
            eb.del_line();
            assert_eq!(text_of(&eb), text, "{text:?} 줄 {line} 을 복제했다 지웠는데 달라졌다");
        }
    }

    /// 위아래로 옮겼다 되돌리면 제자리다(글 끝에 줄바꿈이 있든 없든).
    #[test]
    fn 옮겼다_되돌리면_제자리() {
        for text in ["aaa\nbbb\nccc", "aaa\nbbb\nccc\n", "aaa\nbbb"] {
            for line in 1..3usize {
                let mut eb = buf(text);
                if line >= eb.rope.len_lines() {
                    continue;
                }
                eb.set_cursor(eb.rope.line_to_char(line));
                eb.move_line(true);
                eb.move_line(false);
                assert_eq!(text_of(&eb), text, "{text:?} 줄 {line}");
            }
        }
    }

    /// 맨 위에서 위로, 맨 아래에서 아래로는 아무 일도 없어야 한다.
    #[test]
    fn 끝에서는_안_움직인다() {
        for (line, up) in [(0usize, true), (2, false)] {
            let mut eb = buf("aaa\nbbb\nccc");
            eb.set_cursor(eb.rope.line_to_char(line));
            eb.move_line(up);
            assert_eq!(text_of(&eb), "aaa\nbbb\nccc", "줄 {line} up={up}");
        }
    }

    /// 지운 줄은 되돌리기 **한 번**에 돌아와야 한다(글자마다 쌓이면 안 된다).
    #[test]
    fn 줄_삭제는_되돌리기_한_번() {
        let mut eb = buf("aaa\nbbbbbbbbbb\nccc");
        eb.set_cursor(eb.rope.line_to_char(1));
        eb.del_line();
        assert_eq!(text_of(&eb), "aaa\nccc");
        eb.undo();
        assert_eq!(text_of(&eb), "aaa\nbbbbbbbbbb\nccc");
    }

    /// 옮기기도 되돌리기 **한 번**이다(지웠다 넣으면 두 번이 된다).
    #[test]
    fn 줄_옮기기는_되돌리기_한_번() {
        let mut eb = buf("aaa\nbbb\nccc");
        eb.set_cursor(eb.rope.line_to_char(1));
        eb.move_line(true);
        assert_eq!(text_of(&eb), "bbb\naaa\nccc");
        eb.undo();
        assert_eq!(text_of(&eb), "aaa\nbbb\nccc");
    }
}
