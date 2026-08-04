//! OSC 7/133 시퀀스 스캐너(상태 기계). 본문 파싱은 [`crate::oscparse`].

/// 검출된 OSC 이벤트.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscEvent {
    /// OSC 7: 작업 디렉터리(file:// URI 디코딩 후 경로).
    Cwd(String),
    /// OSC 133;A — 프롬프트 시작.
    PromptStart,
    /// OSC 133;B — 명령 입력 시작.
    CommandStart,
    /// OSC 133;C — 명령 출력 시작.
    CommandExecuted,
    /// OSC 133;D[;exit] — 명령 종료(+종료 코드).
    CommandFinished(Option<i32>),
    /// OSC 52 — 클립보드 복사(디코드된 텍스트).
    ClipboardCopy(String),
    /// OSC 9 — 데스크톱 알림 메시지(iTerm2 계열).
    Notify(String),
    /// OSC 9;4 — 작업 진행률(WT/ConEmu). Some(0..=100) 또는 None(제거/불확정).
    Progress(Option<u8>),
    /// OSC 7771;<verb>;<json> — 제어 평면 in-band 동작(fire-and-forget, opt-in).
    Control(String, String),
    /// OSC 633;E;<base64> — 사용자가 방금 실행한 명령줄(셸 통합 v2). 복원 재실행용.
    CommandLine(String),
}

/// raw 바이트에서 OSC 7/133을 검출한다. ESC ] ... (BEL | ESC \) 를 인식.
#[derive(Default)]
pub struct OscScanner {
    in_osc: bool,
    esc: bool,
    buf: Vec<u8>,
}

impl OscScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// 바이트 청크를 먹이고 검출된 이벤트를 반환한다.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<OscEvent> {
        let mut events = Vec::new();
        for &b in bytes {
            if self.in_osc {
                self.in_body(b, &mut events);
            } else if self.esc && b == b']' {
                self.in_osc = true;
                self.esc = false;
                self.buf.clear();
            } else {
                self.esc = b == 0x1b;
            }
        }
        events
    }

    fn in_body(&mut self, b: u8, events: &mut Vec<OscEvent>) {
        if b == 0x07 {
            self.finish(events);
        } else if self.esc && b == b'\\' {
            self.buf.pop(); // 직전 ESC 제거
            self.finish(events);
        } else {
            self.esc = b == 0x1b;
            self.buf.push(b);
            if self.buf.len() > 4096 {
                self.reset(); // 폭주 방지
            }
        }
    }

    fn finish(&mut self, events: &mut Vec<OscEvent>) {
        if let Ok(s) = std::str::from_utf8(&self.buf) {
            if let Some(e) = crate::oscparse::parse_osc(s) {
                events.push(e);
            }
        }
        self.reset();
    }

    fn reset(&mut self) {
        self.in_osc = false;
        self.esc = false;
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_osc133_finished_with_code() {
        let mut s = OscScanner::new();
        let ev = s.feed(b"\x1b]133;D;0\x07");
        assert_eq!(ev, vec![OscEvent::CommandFinished(Some(0))]);
    }

    #[test]
    fn detects_osc7_cwd_st_terminated() {
        let mut s = OscScanner::new();
        let ev = s.feed(b"\x1b]7;file://host/C:/Users%20x\x1b\\");
        assert_eq!(ev, vec![OscEvent::Cwd("/C:/Users x".into())]);
    }

    #[test]
    fn detects_osc1337_currentdir() {
        let mut s = OscScanner::new();
        let ev = s.feed(b"\x1b]1337;CurrentDir=/home/u\x07");
        assert_eq!(ev, vec![OscEvent::Cwd("/home/u".into())]);
    }

    #[test]
    fn detects_osc9_notification() {
        let mut s = OscScanner::new();
        let ev = s.feed(b"\x1b]9;build done\x07");
        assert_eq!(ev, vec![OscEvent::Notify("build done".into())]);
    }

    #[test]
    fn osc9_progress_parsed() {
        let mut s = OscScanner::new();
        // OSC 9;4;1;50 = 진행률 50%, 9;4;0 = 제거.
        assert_eq!(s.feed(b"\x1b]9;4;1;50\x07"), vec![OscEvent::Progress(Some(50))]);
        assert_eq!(s.feed(b"\x1b]9;4;0\x07"), vec![OscEvent::Progress(None)]);
    }

    #[test]
    fn detects_osc777_notify() {
        let mut s = OscScanner::new();
        let ev = s.feed(b"\x1b]777;notify;Build;done\x07");
        assert_eq!(ev, vec![OscEvent::Notify("Build: done".into())]);
    }

    #[test]
    fn detects_osc52_clipboard_copy() {
        let mut s = OscScanner::new();
        // base64("hi") == "aGk="
        let ev = s.feed(b"\x1b]52;c;aGk=\x07");
        assert_eq!(ev, vec![OscEvent::ClipboardCopy("hi".into())]);
    }
}
