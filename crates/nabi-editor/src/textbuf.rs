//! [`TextData`] 위의 **편집 버퍼** — 커서·선택·되돌리기. 화면과 입력은 textview가 맡는다.
//!
//! ## 커서를 바이트 오프셋으로 둔다
//!
//! rope 편집기(`editbuf`)는 커서를 문서 전역 **char** 오프셋으로 둔다. rope가 char↔줄 변환을
//! O(log n)에 해 주기 때문인데, 우리 문서는 그 변환표를 아예 갖고 있지 않다(그게 메모리를
//! 먹는 부분이다). 대신 줄 시작 **바이트** 오프셋은 [`crate::textindex`]가 이미 갖고 있다.
//!
//! 그래서 커서를 바이트 오프셋 하나로 둔다. 줄 번호는 인덱스 이분 탐색으로 O(log n)에 나오고,
//! 열은 그 줄 문자열 안에서만 세면 된다 — 길어야 수백 바이트다.
//!
//! 바이트 오프셋을 쓰면 UTF-8 경계를 우리가 지켜야 한다. 좌우 이동은 이어지는 바이트
//! (`0b10xx_xxxx`)를 건너뛰고, 넣고 지우는 것은 항상 경계에서만 한다.

use crate::textdata::TextData;


/// 되돌리기 한 칸 — 무엇을 어디서 무엇으로 바꿨는지.
pub(crate) struct Delta {
    pub(crate) at: u64,
    /// 지워진 바이트(되돌릴 때 다시 넣는다).
    pub(crate) removed: Vec<u8>,
    /// 새로 들어간 바이트 수(되돌릴 때 이만큼 지운다).
    pub(crate) ins_len: u64,
    /// 커서를 어디로 되돌릴지.
    pub(crate) caret: u64,
    /// 선택 시작도 함께 되돌린다 — 선택을 지우고 친 것을 되돌리면 선택이 살아나야 한다.
    pub(crate) anchor: u64,
    /// 앞 칸과 한 묶음인가(연속 타자를 한 번에 되돌리려고).
    pub(crate) cont: bool,
}

/// 커서·선택·되돌리기를 갖춘 편집 버퍼.
pub struct TextBuf {
    pub data: TextData,
    /// 커서 위치(바이트). 항상 UTF-8 경계에 있다.
    pub caret: u64,
    /// 선택 시작(바이트). `caret`와 같으면 선택 없음.
    pub anchor: u64,
    pub dirty: bool,
    pub readonly: bool,
    /// 위아래로 움직일 때 유지하려는 열(표시 문자 수) — 짧은 줄을 지나도 원래 열로 돌아온다.
    pub goal_col: Option<usize>,
    /// 다음 프레임에 이 줄로 스크롤(찾기·줄 이동·커서 따라가기).
    pub scroll_to: Option<usize>,
    /// 이 인코딩으로 못 적는 글자를 방금 거절한 시각 — 잠깐 안내를 띄우는 데 쓴다.
    pub refused_at: Option<std::time::Instant>,
    /// 지금까지 본 최장 줄(표시 칸) — 가로 스크롤 범위. 줄어들지 않게 누적한다.
    pub seen_cols: usize,
    pub(crate) undo: Vec<Delta>,
    pub(crate) redo: Vec<Delta>,
    /// 다음 편집을 앞 칸과 묶을지(연속 타자).
    pub(crate) group: bool,
}

/// 오프셋을 그 줄의 **글자 경계**로 맞춘다(줄 끝을 넘으면 줄 끝).
///
/// 예전에는 UTF-8 이어짐 바이트를 세어 앞으로 물러났다. CP949 문서에서는 그 판정이 틀리고,
/// 최악의 경우 파일 끝까지 훑는다(교차 검토 2026-08-25). 줄 안에서만 재면 둘 다 없다.
pub(crate) fn snap(d: &TextData, at: u64) -> u64 {
    let at = at.min(d.total());
    let line = d.line_of(at);
    let starts = d.char_starts(line);
    match starts.binary_search(&at) {
        Ok(_) => at,
        // 글자 한가운데(또는 CRLF 사이)면 바로 앞 경계로 물러난다.
        Err(i) => *starts.get(i.saturating_sub(1)).unwrap_or(&at),
    }
}

/// `at`에서 한 글자 왼쪽/오른쪽 자리. **줄바꿈은 CRLF든 LF든 한 걸음**이다.
pub(crate) fn step_pos(d: &TextData, at: u64, right: bool) -> u64 {
    let line = d.line_of(at);
    let (a, b) = d.line_range(line);
    if right {
        if at >= b {
            // 줄 끝 — 다음 줄 첫 자리로. 줄바꿈 바이트 사이에 멈추지 않는다.
            return if line + 1 < d.lines() { d.line_start(line + 1) } else { d.total() };
        }
        let starts = d.char_starts(line);
        return starts.iter().copied().find(|&s| s > at).unwrap_or(b);
    }
    if at <= a {
        // 줄 처음 — 앞 줄의 **글자** 끝으로(그 줄의 줄바꿈 앞).
        return if line > 0 { d.line_range(line - 1).1 } else { 0 };
    }
    let starts = d.char_starts(line);
    starts.iter().copied().rev().find(|&s| s < at).unwrap_or(a)
}

impl TextBuf {
    pub fn new(data: TextData) -> Self {
        Self {
            data, caret: 0, anchor: 0, dirty: false, readonly: false, goal_col: None,
            scroll_to: None, refused_at: None, seen_cols: 0,
            undo: Vec::new(), redo: Vec::new(), group: false,
        }
    }

    /// 선택 범위(시작, 끝). 선택이 없으면 두 값이 같다.
    pub fn selection(&self) -> (u64, u64) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    /// 커서가 있는 줄 번호.
    pub fn caret_line(&self) -> usize {
        self.data.line_of(self.caret)
    }

    /// 커서의 열(줄 시작부터의 **표시 문자** 수 — 화면 좌표와 맞춘다).
    pub fn caret_col(&self) -> usize {
        let line = self.caret_line();
        let start = self.data.line_start(line);
        let raw = self.data.read(start, (self.caret - start) as usize);
        self.data.decode_len(&raw)
    }

    /// 커서를 옮긴다. `extend`가 참이면 선택을 늘리고, 아니면 선택을 접는다.
    pub fn go(&mut self, to: u64, extend: bool) {
        self.caret = snap(&self.data, to.min(self.data.total()));
        if !extend {
            self.anchor = self.caret;
        }
        self.group = false; // 커서가 움직였으면 다음 타자는 새 되돌리기 칸이다.
    }

    /// 한 글자 왼쪽/오른쪽. 줄바꿈은 CRLF든 LF든 한 걸음이다.
    pub fn step(&mut self, right: bool, extend: bool) {
        let to = step_pos(&self.data, self.caret, right);
        self.go(to, extend);
        self.goal_col = None;
    }

    /// 위/아래 줄로. 열은 `goal_col`을 지키려 애쓴다.
    pub fn step_line(&mut self, down: bool, extend: bool) {
        let line = self.caret_line();
        let want = self.goal_col.unwrap_or_else(|| self.caret_col());
        let next = if down { line + 1 } else { line.wrapping_sub(1) };
        if down && line + 1 >= self.data.lines() || !down && line == 0 {
            return;
        }
        let to = self.data.offset_of_col(next, want);
        self.go(to, extend);
        self.goal_col = Some(want); // go()가 아니라 여기서 되살린다 — 연속 이동에서 열을 잃지 않게.
    }

    /// 줄 처음/끝으로.
    pub fn go_line_edge(&mut self, end: bool, extend: bool) {
        let line = self.caret_line();
        let to = if end { self.data.line_range(line).1 } else { self.data.line_start(line) };
        self.go(to, extend);
        self.goal_col = None;
    }

    /// 편집 묶음을 강제로 끊는다(저장·붙여넣기 등 경계가 분명한 동작 뒤에).
    pub fn break_group(&mut self) {
        self.group = false;
    }

    /// 커서가 화면 밖으로 나갔으면 그 줄로 스크롤을 예약한다.
    ///
    /// `first`/`last`는 지금 보이는 줄 범위다. 커서가 그 안에 있으면 아무 것도 하지 않는다 —
    /// 매 프레임 스크롤을 예약하면 사용자가 마우스로 다른 곳을 볼 수 없다.
    pub fn scroll_to_caret_if_needed(&mut self, first: usize, last: usize) {
        let line = self.caret_line();
        if line < first || line + 1 >= last {
            self.scroll_to = Some(line.saturating_sub(2));
        }
    }

    /// 되돌리기 스택이 들고 있는 바이트(진단·시험용).
    pub fn undo_bytes(&self) -> usize {
        self.undo.iter().map(|d| d.removed.len()).sum()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// 선택한 부분을 문자열로(복사·잘라내기).
    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        if a == b {
            return String::new();
        }
        let raw = self.data.read(a, (b - a) as usize);
        self.data.decode(&raw)
    }
}

#[cfg(test)]
#[path = "textbuf_tests.rs"]
mod tests;
