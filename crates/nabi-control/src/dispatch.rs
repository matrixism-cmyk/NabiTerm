//! 요청 → 응답 라우팅. 읽기(list/capture)는 SharedPanes 직접, 쓰기는 정책 게이트+cmd_tx.

use crate::policy::ControlPolicy;
use crate::protocol::{ControlRequest, ControlResponse, PaneInfo};
use crate::subscribe::EventHub;
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::{AppCtl, Command, Event, ShellKind};
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
        ControlRequest::ListPanes => list_panes(panes),
        ControlRequest::Capture { pane, lines, start, end, escapes } => {
            tracing::info!(target: "control", from = ?from, pane, lines, "capture");
            capture(panes, pane, lines, start, end, escapes)
        }
        // 읽기 전용(capture 동급): 화면을 규칙으로 평가한 근거를 돌려준다(A4).
        ControlRequest::AgentExplain { pane } => crate::explain::agent_explain(panes, pane),
        // ── 쓰기 동작: verb 그룹별 정책 게이트(CP-7 — act/inject 별도 승인) ──
        write_verb => {
            let g = group_of(&write_verb);
            if !policy.allow(g, from) {
                tracing::warn!(target: "control", from = ?from, group = ?g, "거부(정책)");
                let msg = match policy.mode() {
                    crate::policy::Mode::Off => "제어가 꺼져 있음(설정 control.mode)",
                    _ => "승인 대기 — nabiTerm에서 승인 후 다시 시도하세요",
                };
                return err(msg);
            }
            dispatch_write(write_verb, panes, cmd_tx, app_tx, cfg, events, from)
        }
    }
}

/// verb → 권한 그룹(read는 dispatch 전에 분기돼 여기 안 옴).
fn group_of(req: &ControlRequest) -> crate::policy::Group {
    use crate::policy::Group;
    match req {
        ControlRequest::SendInput { .. }
        | ControlRequest::ClosePane { .. }
        | ControlRequest::OpenSftp { .. } => Group::Inject,
        _ => Group::Act,
    }
}

/// 승인된 쓰기 동작 실행(spawn/send/close/resize/open-browser/open-sftp).
fn dispatch_write(
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
        ControlRequest::PaneStatusSet { key, value, ttl_ms } => {
            tracing::info!(target: "control", from = ?from, "status-set");
            app_tx.send(AppCtl::PaneStatus { pane: from.unwrap_or(0), key, value, ttl_ms }).ok(); ControlResponse::Ok
        }
        _ => err("알 수 없는 동작"),
    }
}

fn list_panes(panes: &SharedPanes) -> ControlResponse {
    let Ok(map) = panes.read() else {
        return err("pane 레지스트리 잠금 실패");
    };
    let mut out: Vec<PaneInfo> = map
        .iter()
        .map(|(id, v)| {
            let (cols, rows) = v.model.lock().map(|m| (m.size().cols(), m.size().rows())).unwrap_or((0, 0));
            // 제목 우선순위: 사용자/제어 지정 > OSC 제목 > 스폰 라벨.
            let t = v
                .user_title
                .lock()
                .ok()
                .and_then(|u| u.clone())
                .or_else(|| {
                    v.model.lock().ok().map(|m| m.title().to_string()).filter(|t| !t.is_empty())
                })
                .unwrap_or_else(|| v.title.clone());
            // CP-6: 메타(종류·cwd·활동 상태·exit) 동봉.
            let (kind, cwd, state, state_ms, last_exit) = v
                .meta
                .lock()
                .map(|m| {
                    (m.kind.to_string(), m.cwd.clone(), m.state().to_string(), m.state_ms(), m.last_exit)
                })
                .unwrap_or_else(|_| (String::new(), None, String::new(), 0, None));
            PaneInfo { id: id.get(), title: t, cols, rows, kind, cwd, state, state_ms, last_exit }
        })
        .collect();
    out.sort_by_key(|p| p.id);
    ControlResponse::Panes { panes: out }
}

fn capture(
    panes: &SharedPanes,
    pane: u64,
    lines: u32,
    start: Option<i64>,
    end: Option<i64>,
    escapes: bool,
) -> ControlResponse {
    let Ok(map) = panes.read() else {
        return err("pane 레지스트리 잠금 실패");
    };
    let Some(v) = map.get(&PaneId::new(pane)) else {
        return err(&format!("pane {pane} 없음"));
    };
    let Ok(m) = v.model.lock() else {
        return err("모델 잠금 실패");
    };
    let cur = m.cursor();
    let lines = lines.clamp(1, 10_000) as usize;
    // CP-8: 범위/SGR 캡처 — 음수 인덱스는 끝(총 줄 수)에서 거꾸로.
    let text = if start.is_some() || end.is_some() || escapes {
        let total = m.total_abs_lines() as i64;
        let abs = |v: i64| if v < 0 { total + v } else { v }.clamp(0, total) as usize;
        let e = end.map(abs).unwrap_or(total as usize);
        let s = start.map(abs).unwrap_or(e.saturating_sub(lines)).min(e);
        if escapes {
            m.lines_abs_sgr(s, e, &nabi_vt::Theme::default()).join("\n")
        } else {
            m.lines_abs_text(s, e).join("\n")
        }
    } else {
        m.dump_text(lines)
    };
    ControlResponse::Captured { pane, text, row: cur.row, col: cur.col, alt_screen: m.alt_screen() }
}

/// 설정 문자열 → ShellKind(미상은 기본 PowerShell). workspace의 shell_from_str과 동일 규칙.
fn shell_from_str(s: &str) -> ShellKind {
    match s.to_ascii_lowercase().as_str() {
        "pwsh" => ShellKind::Pwsh,
        "cmd" => ShellKind::Cmd,
        "wsl" => ShellKind::Wsl { distro: None },
        "gitbash" => ShellKind::GitBash,
        _ => ShellKind::WindowsPowerShell,
    }
}

fn err(m: &str) -> ControlResponse {
    ControlResponse::Err { message: m.to_string() }
}
