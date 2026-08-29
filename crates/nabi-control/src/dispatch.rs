//! 요청 → 응답 라우팅. 읽기(list/capture)는 SharedPanes 직접, 쓰기는 정책 게이트+cmd_tx.

use crate::policy::ControlPolicy;
use crate::dispatchread::{err, shell_from_str};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::subscribe::EventHub;
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::{AppCtl, Command, Event};
use nabi_types::{GridSize, PaneId};
use std::sync::atomic::{AtomicU64, Ordering};

/// 스폰 상관 ID 발급기(프로세스 전역) — PaneSpawned.seq와 대조(G1).
static SPAWN_SEQ: AtomicU64 = AtomicU64::new(1);

/// 스폰 기본값(설정에서 주입) — 제어 평면이 셸을 띄울 때 쓴다.
#[derive(Clone)]
pub struct SpawnCfg {
    pub scrollback: usize,
    pub encoding: String,
    pub cols: u16,
    pub rows: u16,
}

/// 인증된 연결의 단발 요청 하나를 처리한다(Wait/Tail 스트림은 server에서 별도).
#[allow(clippy::too_many_arguments)]
pub fn dispatch(
    req: ControlRequest,
    panes: &SharedPanes,
    cmd_tx: &Sender<Command>,
    app_tx: &Sender<AppCtl>,
    policy: &ControlPolicy,
    cfg: &SpawnCfg,
    events: &EventHub,
    from: Option<u64>,
) -> ControlResponse {
    match req {
        ControlRequest::Hello { .. } => ControlResponse::Ok,
        ControlRequest::ListPanes => crate::dispatchread::list_panes(panes),
        // 읽기 전용 진단 — 휠/붙여넣기가 왜 그렇게 도는지 추측하지 않고 확인한다.
        ControlRequest::PaneModes { pane } => crate::dispatchread::pane_modes(panes, pane),
        ControlRequest::Capture { pane, lines, start, end, escapes } => {
            tracing::info!(target: "control", from = ?from, pane, lines, "capture");
            crate::dispatchread::capture(panes, pane, lines, start, end, escapes)
        }
        // 읽기 전용(capture 동급): 화면을 규칙으로 평가한 근거를 돌려준다(A4).
        ControlRequest::AgentExplain { pane } => crate::explain::agent_explain(panes, pane),
        // ── 쓰기 동작: verb 그룹별 정책 게이트(CP-7 — act/inject 별도 승인) ──
        write_verb => crate::gate::gated_write(write_verb, panes, cmd_tx, app_tx, policy, cfg, events, from),
    }
}

/// verb → 권한 그룹(read는 dispatch 전에 분기돼 여기 안 옴).
pub(crate) fn group_of(req: &ControlRequest) -> crate::policy::Group {
    use crate::policy::Group;
    match req {
        ControlRequest::SendInput { .. }
        | ControlRequest::ClosePane { .. }
        | ControlRequest::OpenSftp { .. }
        // 원격/로컬 파일을 실제로 쓰는 전송은 주입 등급(별도 승인).
        | ControlRequest::SftpGet { .. }
        | ControlRequest::SftpPut { .. } => Group::Inject,
        _ => Group::Act,
    }
}

/// SFTP 조작을 앱에 보내고 `SftpCtlDone{seq}` 회신을 기다린다(LayoutExport와 같은 패턴).
fn sftp_roundtrip(
    app_tx: &Sender<AppCtl>,
    events: &EventHub,
    op: nabi_proto::SftpCtlOp,
    timeout_secs: u64,
) -> ControlResponse {
    let seq = SPAWN_SEQ.fetch_add(1, Ordering::Relaxed);
    let rx = events.subscribe();
    app_tx.send(AppCtl::SftpCtl { seq, op }).ok();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            if let Event::SftpCtlDone { seq: s, ok, data } = ev {
                if s == seq {
                    return if ok {
                        ControlResponse::Event { pane: 0, kind: "sftp".into(), data }
                    } else {
                        err(&data)
                    };
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    err("SFTP 조작 시간 초과")
}

/// 승인된 쓰기 동작 실행(spawn/send/close/resize/open-browser/open-sftp).
pub(crate) fn dispatch_write(
    req: ControlRequest,
    panes: &SharedPanes,
    cmd_tx: &Sender<Command>,
    app_tx: &Sender<AppCtl>,
    cfg: &SpawnCfg,
    events: &EventHub,
    from: Option<u64>,
) -> ControlResponse {
    match req {
        ControlRequest::SpawnTerminal { ssh: Some(session), .. } => {
            // SSH 스폰: 자격증명은 앱(볼트/connect_saved) 경유 — 평문 금지 원칙.
            tracing::info!(target: "control", from = ?from, %session, "spawn-ssh");
            app_tx.send(AppCtl::ConnectSession { session }).ok();
            ControlResponse::Ok
        }
        ControlRequest::SpawnTerminal { shell, cwd, dock, ssh: None } => {
            tracing::info!(target: "control", from = ?from, %shell, ?dock, "spawn");
            // 도킹 위치(분할/새 창)는 앱이 다음 PaneSpawned에 적용(CP-7).
            if let Some(d) = dock.filter(|d| d != "tab") {
                app_tx.send(AppCtl::DockNext { dock: d }).ok();
            }
            // G1: 상관 seq를 명령에 실어 보내고 PaneSpawned의 seq 에코로 정확한
            // ID를 회신받는다(동시 스폰에도 뒤바뀜 없음 — before/after 폴링 제거).
            let seq = SPAWN_SEQ.fetch_add(1, Ordering::Relaxed);
            let rx = events.subscribe();
            cmd_tx
                .send(Command::SpawnLocalPane {
                    shell: shell_from_str(&shell),
                    size: GridSize::new(cfg.cols, cfg.rows),
                    scrollback: cfg.scrollback,
                    encoding: cfg.encoding.clone(),
                    cwd,
                    reply_seq: Some(seq),
                })
                .ok();
            for _ in 0..200 {
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        Event::PaneSpawned { pane, seq: Some(s) } if s == seq => {
                            return ControlResponse::Spawned { pane: pane.get() };
                        }
                        // 스폰 실패(예: 설치 안 된 셸)는 즉시 원인을 회신(타임아웃 대기 X).
                        Event::SpawnFailed { seq: Some(s), message } if s == seq => {
                            return err(&message);
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            err("스폰 시간 초과")
        }
        ControlRequest::SendInput { pane, data, raw } => {
            tracing::info!(target: "control", from = ?from, pane, raw, "send-input");
            // G6: 타깃이 bracketed paste 모드면 200~/201~ 래핑 + 내부 종결자 제거
            // (붙여넣기 주입 방지 경로와 동일 의미론). --raw는 제어문자 의도 주입용.
            let Some(bp) = panes.read().ok().and_then(|m| {
                m.get(&PaneId::new(pane))
                    .map(|v| v.model.lock().map(|md| md.bracketed_paste()).unwrap_or(false))
            }) else {
                return err(&format!("pane {pane} 없음"));
            };
            let payload = if bp && !raw {
                format!("\x1b[200~{}\x1b[201~", data.replace("\x1b[201~", ""))
            } else {
                data
            };
            cmd_tx
                .send(Command::WriteInput {
                    pane: PaneId::new(pane),
                    data: bytes::Bytes::from(payload.into_bytes()),
                })
                .ok();
            ControlResponse::Ok
        }
        ControlRequest::ClosePane { pane } => {
            tracing::info!(target: "control", from = ?from, pane, "close");
            cmd_tx.send(Command::ClosePane { pane: PaneId::new(pane) }).ok();
            ControlResponse::Ok
        }
        ControlRequest::Resize { pane, cols, rows } => {
            tracing::info!(target: "control", from = ?from, pane, cols, rows, "resize");
            cmd_tx
                .send(Command::Resize {
                    pane: PaneId::new(pane),
                    size: GridSize::new(cols.max(1), rows.max(1)),
                })
                .ok();
            ControlResponse::Ok
        }
        ControlRequest::OpenBrowser { path } => {
            tracing::info!(target: "control", from = ?from, ?path, "open-browser");
            app_tx.send(AppCtl::OpenBrowser { path }).ok();
            ControlResponse::Ok
        }
        ControlRequest::OpenHere { path } => {
            tracing::info!(target: "control", from = ?from, %path, "open-here");
            app_tx.send(AppCtl::OpenHere { path }).ok();
            ControlResponse::Ok
        }
        ControlRequest::Screenshot { pane, out } => {
            tracing::info!(target: "control", from = ?from, ?pane, "screenshot");
            // 화면은 UI 실만 만질 수 있다. 앱에 시키고 결과 경로는 그쪽이 알린다.
            app_tx.send(AppCtl::Screenshot { pane, out }).ok();
            ControlResponse::Ok
        }
        ControlRequest::Progress { pane, percent } => {
            tracing::info!(target: "control", from = ?from, pane, ?percent, "progress");
            // 진행률은 pane 에 붙는 값이라 오케스트레이터가 아니라 앱 상태로 간다.
            app_tx.send(AppCtl::Progress { pane, percent }).ok();
            ControlResponse::Ok
        }
        ControlRequest::OpenWeb { url, window } => {
            tracing::info!(target: "control", from = ?from, ?url, window, "web");
            app_tx.send(AppCtl::OpenWeb { url, window }).ok();
            ControlResponse::Ok
        }
        ControlRequest::OpenEditor { path } => {
            tracing::info!(target: "control", from = ?from, %path, "open-file");
            app_tx.send(AppCtl::OpenEditor { path }).ok();
            ControlResponse::Ok
        }
        ControlRequest::OpenSftp { session } => {
            tracing::info!(target: "control", from = ?from, %session, "open-sftp");
            app_tx.send(AppCtl::OpenSftp { session }).ok();
            ControlResponse::Ok
        }
        ControlRequest::Focus { pane } => {
            tracing::info!(target: "control", from = ?from, pane, "focus");
            app_tx.send(AppCtl::Focus { pane }).ok();
            ControlResponse::Ok
        }
        ControlRequest::SetTitle { pane, title } => {
            tracing::info!(target: "control", from = ?from, pane, "set-title");
            app_tx.send(AppCtl::SetTitle { pane, title }).ok();
            ControlResponse::Ok
        }
        ControlRequest::Notify { title, body } => {
            tracing::info!(target: "control", from = ?from, "notify");
            app_tx.send(AppCtl::Notify { from, title, body }).ok();
            ControlResponse::Ok
        }
        // 호출 pane(from)의 상태를 설정/삭제 — 자기 자신만 갱신.
        ControlRequest::LayoutExport => {
            // 레이아웃은 앱(UI 스레드) 소유 — seq 상관으로 회신을 기다린다(spawn과 같은 패턴).
            let seq = SPAWN_SEQ.fetch_add(1, Ordering::Relaxed);
            let rx = events.subscribe();
            app_tx.send(AppCtl::LayoutExport { seq }).ok();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                while let Ok(ev) = rx.try_recv() {
                    if let Event::LayoutJson { seq: s, json } = ev {
                        if s == seq {
                            return ControlResponse::Event { pane: 0, kind: "layout".into(), data: json };
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            err("레이아웃 회신 시간 초과")
        }
        // S6-55: SFTP 조작 — 앱(UI 스레드)의 열린 연결로 실행하고 seq 상관으로 회신을 기다린다.
        ControlRequest::SftpList { path } => {
            tracing::info!(target: "control", from = ?from, %path, "sftp-list");
            sftp_roundtrip(app_tx, events, nabi_proto::SftpCtlOp::List { path }, 30)
        }
        ControlRequest::SftpGet { remote, local } => {
            tracing::info!(target: "control", from = ?from, %remote, "sftp-get");
            sftp_roundtrip(app_tx, events, nabi_proto::SftpCtlOp::Get { remote, local }, 600)
        }
        ControlRequest::SftpPut { local, remote } => {
            tracing::info!(target: "control", from = ?from, %remote, "sftp-put");
            sftp_roundtrip(app_tx, events, nabi_proto::SftpCtlOp::Put { local, remote }, 600)
        }
        ControlRequest::ScheduleCreate { name, spec, kind, payload, pane_title } => {
            tracing::info!(target: "control", from = ?from, %spec, "schedule-create");
            app_tx.send(AppCtl::ScheduleCreate { name, spec, kind, payload, pane_title }).ok(); ControlResponse::Ok
        }
        ControlRequest::PaneStatusSet { key, value, ttl_ms } => {
            tracing::info!(target: "control", from = ?from, "status-set");
            app_tx.send(AppCtl::PaneStatus { pane: from.unwrap_or(0), key, value, ttl_ms }).ok(); ControlResponse::Ok
        }
        _ => err("알 수 없는 동작"),
    }
}
