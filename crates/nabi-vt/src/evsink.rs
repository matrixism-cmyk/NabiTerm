//! Term 이벤트 수집 — 제목·벨·질의 응답. grid.rs에서 분리(파일 크기 규율).

use alacritty_terminal::event::{Event, EventListener};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct EvState {
    pub title: String,
    pub bells: usize,
    /// 터미널 질의에 대한 응답 바이트(PTY로 되돌려 써야 한다).
    pub replies: Vec<u8>,
}

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

    /// 현재 제목을 복사해 돌려준다.
    pub(crate) fn title(&self) -> String {
        self.0
            .lock()
            .map(|s| s.title.clone())
            .unwrap_or_default()
    }
}
