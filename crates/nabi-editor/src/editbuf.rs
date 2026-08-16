//! 대용량 파일 편집 엔진(E6) 모델 — ropey rope + 커서/선택 + 인코딩·EOL. 편집·undo는 editbufedit.
//!
//! 내부는 **LF 정규화** 텍스트만 보관(열/줄 계산 단순화). 원본 EOL은 따로 기억해 저장 시 복원한다.

use ropey::Rope;
use std::path::Path;

/// rope 편집기 최대 크기(이 값 초과만 OOM 보호로 읽기 전용 뷰어). 대용량도 기본 편집 가능(사용자 요청).
pub const EDIT_CAP: u64 = 512_000_000;

/// 직전 편집 종류 — 같은 종류 연속 편집은 한 undo 단위로 묶는다(editbufedit).
#[derive(PartialEq, Clone, Copy)]
pub enum EditKind {
    Insert,
    Delete,
}

/// rope 편집 버퍼 — 선택(커서 포함)/undo·redo 스택 + 원본 인코딩·EOL.
pub struct EditBuf {
    pub rope: Rope,
    /// 편집 선택. 지금은 항상 범위 1개지만 자료구조는 멀티커서를 담을 수 있다(editsel).
    pub sel: crate::editsel::Selection,
    pub dirty: bool,
    /// 다음 프레임에 커서를 보이도록 스크롤(키 이동/편집 후 set).
    pub ensure_visible: bool,
    pub enc: String,
    pub eol: &'static str,
    /// 탭 폭(칸) — 표시·탭 스톱 계산 기준(EditorConfig.tab_size).
    pub tab: usize,
    /// Tab 키를 공백으로 넣는다(EditorConfig.indent_spaces).
    pub spaces: bool,
    /// 지금까지 본 최대 표시 열 — 가로 스크롤 범위. 보이는 줄만으로 정하면 스크롤 중
    /// 범위가 요동치므로 줄지 않게 누적한다.
    pub seen_cols: usize,
    pub undo: Vec<(Rope, usize)>,
    pub redo: Vec<(Rope, usize)>,
    /// 현재 undo 묶음이 열려 있는지(연속 편집 누적용 — editbufedit).
    pub undo_open: bool,
    pub last_kind: Option<EditKind>,
    /// 직전 편집 직후의 커서 위치 — 인접 편집인지 판정한다.
    pub last_at: usize,
    /// 직전 편집 시각 — 너무 오래 끊긴 타자는 다른 묶음으로 나눈다.
    pub last_time: Option<std::time::Instant>,
    /// 마지막 저장 시점의 undo 깊이. 되돌려 이 상태로 오면 수정 표시가 사라진다.
    pub saved_depth: Option<usize>,
    /// 구문 강조 무효화 신호(T6-1): 편집마다 증가하는 세대 + 가장 위의 변경 줄.
    /// ropehl이 읽고 소비한다(체크포인트를 그 줄부터 버림).
    pub hl_gen: u64,
    pub hl_dirty_from: usize,
}

impl EditBuf {
    /// LF 정규화 텍스트로 빈 히스토리 버퍼를 만든다(open·테스트 공용).
    pub fn new_buf(lf: &str, enc: String, eol: &'static str) -> EditBuf {
        EditBuf {
            rope: Rope::from_str(lf), sel: crate::editsel::Selection::caret(0), dirty: false,
            ensure_visible: false, enc, eol, tab: nabi_types::DEFAULT_TAB, spaces: true,
            seen_cols: 0, undo: Vec::new(), redo: Vec::new(),
            undo_open: false, last_kind: None, last_at: 0, last_time: None, saved_depth: Some(0),
            hl_gen: 0, hl_dirty_from: 0,
        }
    }

    /// 파일을 읽어 편집 버퍼로 연다(크기 EDIT_CAP 이하). 실패/초과면 None.
    pub fn open(path: &Path) -> Option<EditBuf> {
        let bytes = std::fs::read(path).ok()?;
        if bytes.len() as u64 > EDIT_CAP {
            return None;
        }
        let (text, enc, eol) = crate::editload::decode(&bytes);
        let lf = text.replace("\r\n", "\n").replace('\r', "\n"); // 내부는 LF 통일.
        Some(Self::new_buf(&lf, enc, eol))
    }

    /// 저장용 바이트(원본 EOL로 복원한 UTF-8).
    pub fn to_bytes(&self) -> Vec<u8> {
        let lf = self.rope.to_string();
        let out = match self.eol {
            "CRLF" => lf.replace('\n', "\r\n"),
            "CR" => lf.replace('\n', "\r"),
            _ => lf,
        };
        out.into_bytes()
    }

    /// 구문 강조 무효화 기록(T6-1) — 편집이 시작되는 char 위치의 줄부터 다시 칠해야 한다.
    pub fn mark_hl(&mut self, at_char: usize) {
        self.hl_gen = self.hl_gen.wrapping_add(1);
        let at = at_char.min(self.rope.len_chars());
        self.hl_dirty_from = self.hl_dirty_from.min(self.rope.char_to_line(at));
    }

    /// i번째 줄 문자열(개행 제외 — 렌더/찾기용).
    pub fn line_string(&self, i: usize) -> String {
        if i >= self.rope.len_lines() {
            return String::new();
        }
        self.rope.line(i).to_string().trim_end_matches('\n').to_string()
    }

    /// i번째 줄의 표시 문자 수(개행 제외).
    pub fn line_len(&self, i: usize) -> usize {
        if i >= self.rope.len_lines() {
            return 0;
        }
        let l = self.rope.line(i);
        let n = l.len_chars();
        if n > 0 && l.char(n - 1) == '\n' {
            n - 1
        } else {
            n
        }
    }

    /// 커서 위치(주 범위의 이동단).
    pub fn cursor(&self) -> usize {
        self.sel.primary().head
    }

    /// 커서를 옮긴다(선택 해제 — 캐럿만 남긴다).
    pub fn set_cursor(&mut self, at: usize) {
        self.sel = crate::editsel::Selection::caret(at);
    }

    /// 선택 앵커를 유지한 채 커서만 옮긴다(Shift+이동).
    pub fn move_head(&mut self, to: usize) {
        let anchor = self.sel.primary().anchor;
        self.sel.set_primary(crate::editsel::Range { anchor, head: to });
    }

    /// 현재 선택 범위 [start,end)(없으면 None).
    pub fn selection(&self) -> Option<(usize, usize)> {
        let r = self.sel.primary();
        (!r.is_caret()).then(|| (r.start(), r.end()))
    }

    /// 선택된 텍스트(없으면 빈 문자열).
    pub fn selected_text(&self) -> String {
        // 멀티범위(박스 선택)는 줄바꿈으로 이어 붙인다(컬럼 복사 관행).
        if self.sel.len() > 1 {
            return self
                .sel
                .ranges()
                .iter()
                .filter(|r| !r.is_caret())
                .map(|r| self.rope.slice(r.start()..r.end()).to_string())
                .collect::<Vec<_>>()
                .join("\n");
        }
        match self.selection() {
            Some((a, b)) => self.rope.slice(a..b).to_string(),
            None => String::new(),
        }
    }

    /// 커서 (줄, 열) — 0-base.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.rope.char_to_line(self.cursor());
        (line, self.cursor() - self.rope.line_to_char(line))
    }

    /// 커서가 있는 줄의 표시 형태(탭 확장 + 열 대응).
    pub fn disp_line(&self, line: usize) -> crate::editbufcol::DispLine {
        crate::editbufcol::DispLine::new(&self.line_string(line), self.tab)
    }

    /// 커서 왼쪽 grapheme 경계(줄 처음이면 앞 줄 끝). 결합 문자·이모지를 쪼개지 않는다.
    pub fn step_left(&self) -> usize {
        let c = self.cursor();
        if c == 0 {
            return 0;
        }
        let line = self.rope.char_to_line(c);
        let ls = self.rope.line_to_char(line);
        if c == ls {
            return c - 1; // 개행을 넘어 앞 줄 끝으로.
        }
        ls + crate::editbufcol::grapheme_left(&self.line_string(line), c - ls)
    }

    /// 커서 오른쪽 grapheme 경계(줄 끝이면 다음 줄 처음).
    pub fn step_right(&self) -> usize {
        let c = self.cursor();
        if c >= self.rope.len_chars() {
            return self.rope.len_chars();
        }
        let line = self.rope.char_to_line(c);
        let (ls, s) = (self.rope.line_to_char(line), self.line_string(line));
        if c - ls >= s.chars().count() {
            return c + 1; // 줄 끝(개행) 넘기.
        }
        ls + crate::editbufcol::grapheme_right(&s, c - ls)
    }

    /// 커서 왼쪽 단어 경계(이전 단어 시작). 비단어 건너뛴 뒤 단어를 건너뛴다.
    pub fn word_left(&self) -> usize {
        let mut p = self.cursor();
        while p > 0 && !is_word(self.rope.char(p - 1)) {
            p -= 1;
        }
        while p > 0 && is_word(self.rope.char(p - 1)) {
            p -= 1;
        }
        p
    }

    /// 커서 오른쪽 단어 경계(다음 단어 시작). 단어를 건너뛴 뒤 비단어를 건너뛴다.
    pub fn word_right(&self) -> usize {
        let len = self.rope.len_chars();
        let mut p = self.cursor();
        while p < len && is_word(self.rope.char(p)) {
            p += 1;
        }
        while p < len && !is_word(self.rope.char(p)) {
            p += 1;
        }
        p
    }
}

/// 단어 구성 문자(영숫자·밑줄·CJK 등). 공백·구두점은 단어 경계.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::EditBuf;

    fn buf(s: &str) -> EditBuf {
        EditBuf::new_buf(s, "UTF-8".into(), "LF")
    }

    #[test]
    fn selection_and_line_accessors() {
        let mut b = buf("hi\nworld");
        b.sel = crate::editsel::Selection::single(4, 1); // [1,4) = "i\nw"
        assert_eq!(b.selected_text(), "i\nw");
        assert_eq!(b.line_string(1), "world");
        assert_eq!(b.line_len(0), 2);
        assert_eq!(b.cursor_line_col(), (0, 1));
    }

    #[test]
    fn eol_preserved_on_save() {
        let mut b = buf("a\nb");
        b.eol = "CRLF";
        assert_eq!(b.to_bytes(), b"a\r\nb");
        b.eol = "LF";
        assert_eq!(b.to_bytes(), b"a\nb");
    }
}
