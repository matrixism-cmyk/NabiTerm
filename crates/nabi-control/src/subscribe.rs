//! 이벤트 허브(Wait/Tail 구독) — 앱이 오케스트레이터 이벤트를 fan-out하고,
//! 제어 연결이 구독해 조건 충족까지 폴링(비차단)으로 스트림한다.

use crate::protocol::ControlResponse;
use crossbeam_channel::{Receiver, Sender};
use nabi_orchestrator::SharedPanes;
use nabi_proto::Event;
use nabi_types::PaneId;
use std::sync::{Arc, Mutex};

/// 구독자 채널 목록. 앱이 매 프레임 이벤트를 publish, 구독자는 recv.
#[derive(Clone, Default)]
pub struct EventHub {
    subs: Arc<Mutex<Vec<Sender<Event>>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// 앱(UI 스레드)이 드레인한 이벤트를 구독자에게 복제 전달. 끊긴 채널은 청소.
    pub fn publish(&self, ev: &Event) {
        if let Ok(mut subs) = self.subs.lock() {
            if subs.is_empty() {
                return;
            }
            subs.retain(|s| s.send(ev.clone()).is_ok());
        }
    }

    /// 제어 연결이 구독을 등록(수신 채널 반환).
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Ok(mut subs) = self.subs.lock() {
            subs.push(tx);
        }
        rx
    }
}

/// Wait 조건. 문자열에서 파싱(미상은 Exit).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WaitCond {
    Exit,
    CommandDone,
    Output,
    Idle,
}

impl WaitCond {
    pub fn parse(s: &str) -> WaitCond {
        match s {
            "command-done" => WaitCond::CommandDone,
            "output" => WaitCond::Output,
            "idle" => WaitCond::Idle,
            _ => WaitCond::Exit,
        }
    }
}

/// 이벤트가 대상 pane의 조건을 충족하면 회신 페이로드(JSON)를 돌려준다(G3).
/// Idle은 여기서 판정 안 함 — 타이머.
fn payload(ev: &Event, pane: u64, cond: WaitCond) -> Option<String> {
    match (cond, ev) {
        (WaitCond::Exit, Event::PaneExited { pane: p, code }) if p.get() == pane => {
            Some(serde_json::json!({ "code": code }).to_string())
        }
        (WaitCond::Exit, Event::SshDisconnected { pane: p, message }) if p.get() == pane => {
            Some(serde_json::json!({ "error": message }).to_string())
        }
        (WaitCond::CommandDone, Event::CommandBlock { pane: p, block }) if p.get() == pane => {
            serde_json::to_string(block).ok()
        }
        (WaitCond::Output, Event::PaneOutput { pane: p }) if p.get() == pane => {
            Some(String::new())
        }
        _ => None,
    }
}

/// 대상 pane의 출력 이벤트인지(Tail·Idle 타이머용).
fn is_output(ev: &Event, pane: u64) -> bool {
    matches!(ev, Event::PaneOutput { pane: p } if p.get() == pane)
}

/// Wait: 조건 충족 또는 타임아웃까지 비차단 폴링. 충족 시 Event 응답, 아니면 Err(timeout).
pub async fn run_wait(
    hub: &EventHub,
    panes: &SharedPanes,
    pane: u64,
    cond: WaitCond,
    timeout_ms: u64,
) -> ControlResponse {
    let rx = hub.subscribe();
    // 구독 직후 존재 확인: Exit인데 이미 사라졌으면 즉시 충족(구독 전 종료 레이스 방지).
    if cond == WaitCond::Exit
        && !panes.read().map(|m| m.contains_key(&PaneId::new(pane))).unwrap_or(true)
    {
        // 이미 종료(구독 전) — 코드는 알 수 없음을 명시(JSON 일관성, G3).
        return ControlResponse::Event {
            pane,
            kind: "exit".into(),
            data: r#"{"code":null}"#.into(),
        };
    }
    let deadline_steps = (timeout_ms / 50).max(1);
    let mut idle_since = 0u64; // Idle: 마지막 출력 이후 경과(50ms 단위).
    for _ in 0..deadline_steps {
        let mut saw_output = false;
        while let Ok(ev) = rx.try_recv() {
            if let Some(data) = payload(&ev, pane, cond) {
                return ControlResponse::Event { pane, kind: cond_kind(cond).into(), data };
            }
            if is_output(&ev, pane) {
                saw_output = true;
            }
        }
        if cond == WaitCond::Idle {
            idle_since = if saw_output { 0 } else { idle_since + 1 };
            // 500ms(10스텝) 동안 출력 없음 = idle.
            if idle_since >= 10 {
                return ControlResponse::Event { pane, kind: "idle".into(), data: String::new() };
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    ControlResponse::Err { message: "대기 시간 초과".into() }
}

fn cond_kind(c: WaitCond) -> &'static str {
    match c {
        WaitCond::Exit => "exit",
        WaitCond::CommandDone => "command-done",
        WaitCond::Output => "output",
        WaitCond::Idle => "idle",
    }
}
