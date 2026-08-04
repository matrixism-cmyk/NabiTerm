//! UI가 오케스트레이터를 다루는 핸들.

use crate::pane_registry::SharedPanes;
use crossbeam_channel::{Receiver, Sender};
use nabi_proto::{Command, Event};

/// UI 측 오케스트레이터 핸들: 명령 전송 + 이벤트 수신 + 공유 pane 읽기.
pub struct OrchestratorHandle {
    pub cmd_tx: Sender<Command>,
    pub event_rx: Receiver<Event>,
    pub panes: SharedPanes,
}

impl OrchestratorHandle {
    /// 명령을 보낸다(오케스트레이터가 닫혔으면 무시).
    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// 대기 중인 이벤트를 비차단으로 모두 가져온다.
    pub fn drain_events(&self) -> Vec<Event> {
        self.event_rx.try_iter().collect()
    }
}
