//! 대용량 파일 편집 엔진(E6) 모델 — ropey rope + 커서/선택 + 인코딩·EOL. 편집·undo는 editbufedit.
//!
//! 내부는 **LF 정규화** 텍스트만 보관(열/줄 계산 단순화). 원본 EOL은 따로 기억해 저장 시 복원한다.

use ropey::Rope;
use std::path::Path;

/// rope 편집기 최대 크기(이 값 초과만 OOM 보호로 읽기 전용 뷰어). 대용량도 기본 편집 가능(사용자 요청).
pub(crate) const EDIT_CAP: u64 = 512_000_000;

/// 직전 편집 종류 — 같은 종류 연속 편집은 한 undo 단위로 묶는다(editbufedit).
#[derive(PartialEq, Clone, Copy)]
pub(crate) enum EditKind {
    Insert,
    Delete,
}

/// rope 편집 버퍼 — 커서/선택(앵커)/undo·redo 스택 + 원본 인코딩·EOL.
pub(crate) struct EditBuf {
    pub rope: Rope,
    /// 커서 char 인덱스.
    pub cursor: usize,
    /// 선택 앵커(없으면 선택 없음). cursor와 다르면 [min,max)가 선택.
    pub anchor: Option<usize>,
    pub dirty: bool,
    /// 다음 프레임에 커서를 보이도록 스크롤(키 이동/편집 후 set).
    pub ensure_visible: bool,
    pub enc: String,
    pub eol: &'static str,
    pub(crate) undo: Vec<(Rope, usize)>,
    pub(crate) redo: Vec<(Rope, usize)>,
    /// 현재 undo 묶음이 열려 있는지(연속 편집 누적용 — editbufedit).
    pub(crate) undo_open: bool,
    pub(crate) last_kind: Option<EditKind>,
}

impl EditBuf {
    /// LF 정규화 텍스트로 빈 히스토리 버퍼를 만든다(open·테스트 공용).
    pub(crate) fn new_buf(lf: &str, enc: String, eol: &'static str) -> EditBuf {
        EditBuf {
            rope: Rope::from_str(lf), cursor: 0, anchor: None, dirty: false,
            ensure_visible: false, enc, eol, undo: Vec::new(), redo: Vec::new(),
            undo_open: false, last_kind: None,
        }
    }

    /// 파일을 읽어 편집 버퍼로 연다(크기 EDIT_CAP 이하). 실패/초과면 None.
    pub(crate) fn open(path: &Path) -> Option<EditBuf> {
        let bytes = std::fs::read(path).ok()?;
        if bytes.len() as u64 > EDIT_CAP {
            return None;
        }
        let (text, enc, eol) = crate::editload::decode(&bytes);
        let lf = text.replace("\r\n", "\n").replace('\r', "\n"); // 내부는 LF 통일.
        Some(Self::new_buf(&lf, enc, eol))
    }

    /// 저장용 바이트(원본 EOL로 복원한 UTF-8).
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let lf = self.rope.to_string();
        let out = match self.eol {
            "CRLF" => lf.replace('\n', "\r\n"),
            "CR" => lf.replace('\n', "\r"),
            _ => lf,
        };
        out.into_bytes()
    }

    /// i번째 줄 문자열(개행 제외 — 렌더/찾기용).
    pub(crate) fn line_string(&self, i: usize) -> String {
        if i >= self.rope.len_lines() {
            return String::new();
        }
        self.rope.line(i).to_string().trim_end_matches('\n').to_string()
    }

    /// i번째 줄의 표시 문자 수(개행 제외).
    pub(crate) fn line_len(&self, i: usize) -> usize {
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

    /// 현재 선택 범위 [start,end)(없으면 None).
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        match self.anchor {
            Some(a) if a != self.cursor => Some((a.min(self.cursor), a.max(self.cursor))),
            _ => None,
        }
    }

    /// 선택된 텍스트(없으면 빈 문자열).
    pub(crate) fn selected_text(&self) -> String {
        match self.selection() {
            Some((a, b)) => self.rope.slice(a..b).to_string(),
            None => String::new(),
        }
    }

    /// 커서 (줄, 열) — 0-base.
    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        let line = self.rope.char_to_line(self.cursor);
        (line, self.cursor - self.rope.line_to_char(line))
    }

    /// 커서 왼쪽 단어 경계(이전 단어 시작). 비단어 건너뛴 뒤 단어를 건너뛴다.
    pub(crate) fn word_left(&self) -> usize {
        let mut p = self.cursor;
        while p > 0 && !is_word(self.rope.char(p - 1)) {
            p -= 1;
        }
        while p > 0 && is_word(self.rope.char(p - 1)) {
            p -= 1;
        }
        p
    }

    /// 커서 오른쪽 단어 경계(다음 단어 시작). 단어를 건너뛴 뒤 비단어를 건너뛴다.
    pub(crate) fn word_right(&self) -> usize {
        let len = self.rope.len_chars();
        let mut p = self.cursor;
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
        b.cursor = 1;
        b.anchor = Some(4); // [1,4) = "i\nw"
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
