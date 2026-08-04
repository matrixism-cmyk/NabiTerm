//! 로컬 pane 스폰.

use crate::pane_registry::{decoder_for, PaneRuntime, PaneView, SharedPanes};
use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_osc::OscScanner;
use nabi_proto::{Event, ShellKind, SshParams};
use nabi_types::{next_pane_id, GridSize, PaneId};
use nabi_vt::TermModel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

/// ConPTY 스폰 완료 통지(액터가 명령 순서(seq)대로 등록·PaneSpawned 방출).
pub struct SpawnDone {
    pub seq: u64,
    pub pane: PaneId,
    pub label: String,
    pub size: GridSize,
    pub scrollback: usize,
    pub encoding: String,
    /// 요청자 상관 ID(PaneSpawned에 에코 — 제어 평면 G1).
    pub reply_seq: Option<u64>,
    pub result: std::io::Result<nabi_pty::LocalPty>,
}

/// ConPTY 생성(블로킹)을 별도 스레드에서 시작한다 — 여러 탭 복원 시 병렬 스폰.
/// 완료는 done_tx로 통지되고, 등록·이벤트는 액터가 seq 순서대로 처리한다.
#[allow(clippy::too_many_arguments)]
pub fn begin_local_spawn(
    shell: ShellKind,
    size: GridSize,
    scrollback: usize,
    encoding: String,
    cwd: Option<String>,
    reply_seq: Option<u64>,
    seq: u64,
    out_tx: Sender<(PaneId, Bytes)>,
    done_tx: Sender<SpawnDone>,
    exit_tx: Sender<(PaneId, Option<i32>)>,
) -> PaneId {
    let pane = next_pane_id();
    let label = shell.label().to_string();
    let _ = std::thread::Builder::new()
        .name(format!("spawn-{}", pane.get()))
        .spawn(move || {
            // 셸 종료 시 액터에 통지 → 레지스트리 정리 + PaneExited 방출(중앙 처리).
            let on_exit: Box<dyn FnOnce(Option<i32>) + Send> = Box::new(move |code| {
                let _ = exit_tx.send((pane, code));
            });
            let result = nabi_pty::spawn_local(pane, &shell, size, out_tx, cwd.as_deref(), on_exit);
            let _ = done_tx.send(SpawnDone {
                seq,
                pane,
                label,
                size,
                scrollback,
                encoding,
                reply_seq,
                result,
            });
        });
    pane
}

/// 스폰 완료를 레지스트리에 등록하고 PaneSpawned/Error를 방출한다(액터 스레드).
pub fn finish_local_spawn(
    d: SpawnDone,
    state: &mut HashMap<PaneId, PaneRuntime>,
    panes: &SharedPanes,
    event_tx: &Sender<Event>,
) {
    match d.result {
        Ok(pty) => {
            // scrollback 0 = 사실상 무제한(대용량).
            let sb = if d.scrollback == 0 { 100_000 } else { d.scrollback };
            let model = Arc::new(Mutex::new(TermModel::new(d.size, sb)));
            crate::pane_registry::panes_write(panes)
                .insert(d.pane, PaneView::new(model, d.label, "local"));
            state.insert(
                d.pane,
                PaneRuntime {
                    transport: Box::new(pty),
                    osc: OscScanner::new(),
                    decoder: decoder_for(&d.encoding),
                },
            );
            let _ = event_tx.send(Event::PaneSpawned { pane: d.pane, seq: d.reply_seq });
        }
        Err(e) => {
            // seq를 실어 보내 제어 평면이 타임아웃 대신 즉시 원인을 회신하게 한다.
            let _ = event_tx.send(Event::SpawnFailed {
                seq: d.reply_seq,
                message: format!("셸 스폰 실패: {e}"),
            });
        }
    }
}

/// SSH 원격 pane을 만들어 레지스트리에 등록한다(출력은 출력 버스로 비동기 유입).
#[allow(clippy::too_many_arguments)]
pub fn connect_ssh_pane(
    rt: &Handle,
    params: SshParams,
    size: GridSize,
    scrollback: usize,
    encoding: &str,
    state: &mut HashMap<PaneId, PaneRuntime>,
    panes: &SharedPanes,
    out_tx: &Sender<(PaneId, Bytes)>,
    event_tx: &Sender<Event>,
    verifier: std::sync::Arc<crate::hostkey::OrchVerifier>,
    stats_secs: u64,
    reply_seq: Option<u64>,
) {
    let pane = next_pane_id();
    let title = format!("ssh {}@{}", params.user, params.host);
    let known_hosts = nabi_config::StorageLayout::resolve().known_hosts;
    if let Some(parent) = known_hosts.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 세션 종료 통지: 정상 종료(exit)는 PaneExited, 오류 끊김은 SshDisconnected(재연결 제안).
    let ev = event_tx.clone();
    let on_close: Box<dyn FnOnce(Option<String>) + Send> = Box::new(move |err| {
        let _ = match err {
            None => ev.send(Event::PaneExited { pane, code: Some(0) }),
            Some(message) => ev.send(Event::SshDisconnected { pane, message }),
        };
    });
    // 서버 통계 폴링(설정에서 주기 지정, 0=비활성). 별도 exec 채널로 흐른다.
    let stats_poll = (stats_secs > 0)
        .then(|| (event_tx.clone(), std::time::Duration::from_secs(stats_secs)));
    let channel = nabi_ssh::connect(
        rt, pane, params, size, out_tx.clone(), known_hosts, Some(verifier), on_close, stats_poll,
    );
    let sb = if scrollback == 0 { 100_000 } else { scrollback };
    let model = Arc::new(Mutex::new(TermModel::new(size, sb)));
    crate::pane_registry::panes_write(panes).insert(pane, PaneView::new(model, title, "ssh"));
    state.insert(
        pane,
        PaneRuntime {
            transport: Box::new(channel),
            osc: OscScanner::new(),
            decoder: decoder_for(encoding),
        },
    );
    let _ = event_tx.send(Event::PaneSpawned { pane, seq: reply_seq });
}
