//! Term 이벤트 수집 — 제목·벨·질의 응답. grid.rs에서 분리(파일 크기 규율).

use alacritty_terminal::event::{Event, EventListener};
use std::sync::{Arc, Mutex};

pub(crate) struct EvState {
    pub title: String,
    pub bells: usize,
    /// 터미널 질의에 대한 응답 바이트(PTY로 되돌려 써야 한다).
    pub replies: Vec<u8>,
    /// OSC 10/11(전경·배경색) 질의에 답할 색 — 앱이 현재 테마를 넣어 준다.
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
}

impl Default for EvState {
    fn default() -> Self {
        // 색을 모르는 동안에도 답은 해야 한다(무응답이 제일 나쁘다). 어두운 배경을 가정.
        Self {
            title: String::new(),
            bells: 0,
            replies: Vec::new(),
            fg: (0xff, 0xff, 0xff),
            bg: (0x00, 0x00, 0x00),
        }
    }
}

/// alacritty가 쓰는 색 인덱스 — 팔레트 256색 뒤에 전경·배경이 붙는다.
const IDX_FG: usize = 256;
const IDX_BG: usize = 257;

/// Term 이벤트 리스너 — 제목·벨·질의 응답을 수집한다.
#[derive(Clone, Default)]
pub(crate) struct EvSink(pub(crate) Arc<Mutex<EvState>>);

impl EventListener for EvSink {
    fn send_event(&self, ev: Event) {
        // 잠금이 오염돼도 계속 수집한다(제목·응답이 조용히 멈추지 않게).
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match ev {
            Event::Title(t) => s.title = t,
            Event::ResetTitle => s.title.clear(),
            Event::Bell => s.bells += 1,
            // 장치 속성(DA1)·커서 위치(DSR)·키보드 모드·문자 단위 크기 질의의 응답.
            // 버리면 질의한 프로그램이 응답을 기다리다 타임아웃한다.
            Event::PtyWrite(t) => s.replies.extend_from_slice(t.as_bytes()),
            // OSC 10/11 색 질의. 답하지 않으면 배경색으로 밝기 테마를 고르는 TUI가 응답을
            // 기다리다 멈추거나 잘못 짐작한다(우리가 어두운 테마인데 밝은 색으로 그린다).
            Event::ColorRequest(index, fmt) => {
                let c = match index {
                    IDX_FG => s.fg,
                    IDX_BG => s.bg,
                    _ => return, // 팔레트 개별 색 질의는 아직 답하지 않는다.
                };
                let reply = fmt(alacritty_terminal::vte::ansi::Rgb { r: c.0, g: c.1, b: c.2 });
                s.replies.extend_from_slice(reply.as_bytes());
            }
            _ => {}
        }
    }
}

impl EvSink {
    /// 수집된 질의 응답을 꺼낸다(호출측이 PTY로 써야 함).
    pub(crate) fn take_replies(&self) -> Vec<u8> {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut s.replies)
    }

    /// 누적 벨 횟수.
    pub(crate) fn bells(&self) -> usize {
        self.0.lock().map(|s| s.bells).unwrap_or(0)
    }

    /// 색 질의에 답할 전경·배경색을 갱신한다(테마 변경 시).
    pub(crate) fn set_colors(&self, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        if let Ok(mut s) = self.0.lock() {
            s.fg = fg;
            s.bg = bg;
        }
    }

    /// 현재 제목을 복사해 돌려준다.
    pub(crate) fn title(&self) -> String {
        self.0
            .lock()
            .map(|s| s.title.clone())
            .unwrap_or_default()
    }
}
