//! 출력 바이트 라우팅: VT 모델 주입 + OSC 탭 + UI 깨우기.

use crate::pane_registry::{PaneRuntime, SharedPanes};
use crossbeam_channel::Sender;
use nabi_osc::OscEvent;
use nabi_proto::{CommandBlock, Event};
use nabi_types::PaneId;
use std::collections::HashMap;

/// 한 pane의 출력 청크를 처리한다.
pub fn route_output(
    pane: PaneId,
    bytes: &[u8],
    state: &mut HashMap<PaneId, PaneRuntime>,
    panes: &SharedPanes,
    event_tx: &Sender<Event>,
) {
    // 0) pane 인코딩이 비UTF-8이면 UTF-8로 디코드(스트리밍).
    let decoded = state.get_mut(&pane).and_then(|rt| rt.decode(bytes));
    let feed: &[u8] = decoded.as_deref().unwrap_or(bytes);

    // 0.5) trzsz 파일 전송 필터 — VT에 넣기 **전에** 가로챈다. 전송 중에는 화면에
    // 아무것도 보내지 않고(프로토콜이 새면 화면이 깨진다), 회신은 아래 1b와 같은 길로 나간다.
    let filtered = state.get_mut(&pane).map(|rt| rt.trzsz.filter(pane, feed, event_tx));
    let feed: &[u8] = match &filtered {
        Some(r) => {
            if !r.reply.is_empty() {
                if let Some(rt) = state.get_mut(&pane) {
                    let _ = rt.transport.write(&r.reply);
                }
            }
            &r.display
        }
        None => feed,
    };
    if feed.is_empty() {
        // 전송이 다 삼켰다 — UI만 깨워 진행률을 다시 그리게 한다.
        let _ = event_tx.send(Event::PaneOutput { pane });
        return;
    }

    // 1) 권위 있는 VT 모델에 먹인다 + 메타(활동 상태) 갱신.
    // 잠금이 오염돼도 복구해서 계속 쓴다 — 건너뛰면 그 pane은 출력이 영영 멈춘다.
    let mut replies = Vec::new();
    if let Some(view) = crate::pane_registry::panes_read(panes).get(&pane) {
        {
            let mut model = crate::pane_registry::model_lock(&view.model);
            model.process(feed);
            model.push_detect_sample(bytes); // 디코드 전 raw 표본(인코딩 자동 감지, B9).
            replies = model.take_replies(); // 장치 속성·커서 위치 등 질의 응답.
        }
        let mut meta = view.meta.lock().unwrap_or_else(|e| e.into_inner());
        meta.on_output();
    }
    // 1b) 질의 응답을 PTY로 돌려보낸다 — 안 보내면 질의한 프로그램이 타임아웃한다.
    if !replies.is_empty() {
        if let Some(rt) = state.get_mut(&pane) {
            let _ = rt.transport.write(&replies);
        }
    }

    // 2) OSC 7/133 탭에서 명령 경계/cwd 이벤트를 뽑는다.
    if let Some(rt) = state.get_mut(&pane) {
        for ev in rt.osc.feed(feed) {
            // 명령 경계를 모델에 기록: 133;A=프롬프트 절대 위치(점프), 133;D=종료코드(블록 바).
            if matches!(ev, OscEvent::PromptStart | OscEvent::CommandFinished(_)) {
                if let Some(view) = crate::pane_registry::panes_read(panes).get(&pane) {
                    let mut m = crate::pane_registry::model_lock(&view.model);
                    match &ev {
                        OscEvent::PromptStart => m.mark_prompt(),
                        OscEvent::CommandFinished(code) => m.mark_command_done(*code),
                        _ => {}
                    }
                }
            }
            update_meta(pane, &ev, panes);
            emit_osc(pane, ev, event_tx);
        }
    }

    // 3) 해당 pane을 띄운 뷰포트가 repaint 하도록 깨운다.
    let _ = event_tx.send(Event::PaneOutput { pane });
}

/// OSC 이벤트로 pane 메타(cwd·실행 중 명령·exit)를 갱신한다(CP-6).
fn update_meta(pane: PaneId, ev: &OscEvent, panes: &SharedPanes) {
    let Ok(map) = panes.read() else { return };
    let Some(view) = map.get(&pane) else { return };
    let Ok(mut meta) = view.meta.lock() else { return };
    match ev {
        OscEvent::Cwd(path) => meta.on_cwd(path.clone()),
        OscEvent::CommandExecuted => meta.on_command_started(),
        OscEvent::CommandFinished(code) => meta.on_command_done(*code),
        _ => {}
    }
}

fn emit_osc(pane: PaneId, ev: OscEvent, event_tx: &Sender<Event>) {
    match ev {
        OscEvent::Cwd(path) => {
            let _ = event_tx.send(Event::CwdChanged { pane, path });
        }
        OscEvent::CommandFinished(code) => {
            let block = CommandBlock {
                exit_code: code,
                ..Default::default()
            };
            let _ = event_tx.send(Event::CommandBlock { pane, block });
        }
        OscEvent::ClipboardCopy(text) => {
            let _ = event_tx.send(Event::ClipboardCopy { text });
        }
        OscEvent::Notify(text) => {
            let _ = event_tx.send(Event::Notify { pane, text });
        }
        OscEvent::Progress(percent) => {
            let _ = event_tx.send(Event::Progress { pane, percent });
        }
        OscEvent::CommandExecuted => {
            let _ = event_tx.send(Event::CommandStarted { pane });
        }
        OscEvent::Control(verb, json) => {
            let _ = event_tx.send(Event::ControlOsc { pane, verb, json });
        }
        OscEvent::CommandLine(cmd) => {
            let _ = event_tx.send(Event::CommandLine { pane, cmd });
        }
        OscEvent::PromptStart | OscEvent::CommandStart => {}
    }
}
