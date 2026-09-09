//! 직렬 콘솔 pane 열기.
//!
//! 로컬 셸은 ConPTY 만드는 데 시간이 걸려 곁 스레드로 미루지만(`spawn_pane`), 직렬은
//! **여는 것이 곧 붙는 것**이다 — 핸드셰이크도 인증도 없고, 실패해도 즉시 실패한다.
//! 그래서 여기서 바로 열고 결과를 그 자리에서 보낸다.
//!
//! 등록한 뒤는 로컬 셸과 완전히 같다. 라우터는 전송이 무엇인지 묻지 않는다.

use crate::pane_registry::{decoder_for, PaneRuntime, PaneView, SharedPanes};
use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_osc::OscScanner;
use nabi_proto::Event;
use nabi_serial::SerialCfg;
use nabi_types::{next_pane_id, GridSize, PaneId};
use nabi_vt::TermModel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 열 때 필요한 것들 — 인자가 일곱이 되어 자루에 담는다.
pub struct SerialSpawn {
    pub port: String,
    pub baud: u32,
    pub frame: String,
    pub size: GridSize,
    pub scrollback: usize,
    pub encoding: String,
    pub reply_seq: Option<u64>,
}

/// 포트를 열어 pane 으로 등록한다. 실패하면 `SpawnFailed` 로 까닭을 보낸다.
pub fn open_serial_pane(
    s: SerialSpawn,
    state: &mut HashMap<PaneId, PaneRuntime>,
    panes: &SharedPanes,
    out_tx: &Sender<(PaneId, Bytes)>,
    event_tx: &Sender<Event>,
) {
    let pane = next_pane_id();
    // 표기가 이상하면 기본(8N1)으로 조용히 넘어가지 않는다 — 사용자가 적은 것과 다른
    // 설정으로 붙으면 글자가 깨져 나오는데 이유를 알 수 없다.
    let Some((data_bits, parity, stop_bits)) = SerialCfg::parse_frame(&s.frame) else {
        let _ = event_tx.send(Event::SpawnFailed {
            seq: s.reply_seq,
            message: nabi_i18n::trc("serial.badframe").to_string(),
        });
        return;
    };
    let cfg = SerialCfg { baud: s.baud, data_bits, parity, stop_bits };
    match nabi_serial::open(pane, &s.port, cfg, out_tx.clone()) {
        Ok(ch) => {
            // 제목에 설정을 함께 적는다 — 콘솔은 속도를 잘못 잡으면 글자가 깨지는데,
            // 그때 가장 먼저 보고 싶은 것이 지금 무엇으로 붙어 있는가다.
            let label = format!("{} {} {}", s.port, s.baud, cfg.frame());
            let sb = if s.scrollback == 0 { 100_000 } else { s.scrollback };
            let model = Arc::new(Mutex::new(TermModel::new(s.size, sb)));
            crate::pane_registry::panes_write(panes)
                .insert(pane, PaneView::new(model, label, "serial"));
            state.insert(
                pane,
                PaneRuntime {
                    transport: Box::new(ch),
                    osc: OscScanner::new(),
                    decoder: decoder_for(&s.encoding),
                    trzsz: Default::default(),
                },
            );
            let _ = event_tx.send(Event::PaneSpawned { pane, seq: s.reply_seq });
        }
        Err(e) => {
            // 아래 크레이트가 코드로 실어 보낸 것을 여기서 화면 말로 옮긴다(T8-1).
            let _ = event_tx.send(Event::SpawnFailed {
                seq: s.reply_seq,
                message: nabi_i18n::tr_io_current(&e),
            });
        }
    }
}
