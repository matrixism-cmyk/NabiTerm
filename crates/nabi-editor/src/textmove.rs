//! 초대용량 편집기의 **이동**(배치 AG) — 줄 번호로 가기, 찾은 자리 선택하기.
//!
//! `textbuf` 에서 갈라 나왔다(줄 한도). 커서를 옮기는 일 중에서도 **한 번에 멀리 가는**
//! 것들이 여기 모인다 — 화살표 한 칸 이동(`textbuf`)과 성격이 다르고, 둘 다 화면을
//! 함께 옮겨야 한다는 공통점이 있다.

use crate::textbuf::TextBuf;

impl TextBuf {
    /// `[from, to)` 를 선택하고 화면을 그리로 보낸다 — 찾기 결과 표시용(배치 AG).
    ///
    /// 찾은 자리를 **선택까지** 해 주는 이유: 커서만 옮기면 사용자가 무엇이 걸렸는지 눈으로
    /// 확인해야 한다. 선택돼 있으면 바로 복사하거나 덮어쓸 수 있다.
    pub fn select_range(&mut self, from: u64, to: u64) {
        self.go(from, false);
        self.go(to, true);
        self.goal_col = None;
        self.scroll_to = Some(self.caret_line().saturating_sub(2));
    }

    /// 그 줄로 커서를 옮기고 화면도 그리로 보낸다 — **줄 번호로 이동**(배치 AC).
    ///
    /// 수 GB 파일에서 8백만째 줄을 마우스로 찾아가는 것은 사실상 불가능하다. 그런데 인덱스가
    /// 줄 위치를 이미 알고 있어서 이 이동은 파일 크기와 무관하게 값이 같다 — 안 할 이유가 없었다.
    ///
    /// 범위를 넘어서면 **마지막 줄로** 간다. 아무 일도 안 하면 사용자는 자기가 잘못 눌렀는지
    /// 프로그램이 무시했는지 알 수 없다. 끝으로 보내면 적어도 "여기가 끝"이라는 답이 된다.
    ///
    /// 화면은 두 줄 위에서 시작한다. 찾던 줄이 맨 위 첫 줄에 딱 붙으면 앞뒤 맥락이 안 보인다.
    pub fn go_to_line(&mut self, line0: usize, col: Option<usize>) {
        let line = line0.min(self.data.lines().saturating_sub(1));
        let to = match col {
            Some(c) => self.data.offset_of_col(line, c),
            None => self.data.line_start(line),
        };
        self.go(to, false);
        self.goal_col = None;
        self.scroll_to = Some(line.saturating_sub(2));
    }
}
