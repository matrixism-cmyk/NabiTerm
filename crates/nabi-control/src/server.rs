//! named pipe NDJSON 제어 서버 — 전용 스레드의 current_thread tokio에서 수락 루프.

use crate::dispatch::SpawnCfg;
use crate::policy::ControlPolicy;
use crate::protocol::{ControlRequest, ControlResponse};
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// 서버가 연결을 처리하는 데 필요한 공유 자원 묶음.
#[derive(Clone)]
pub struct ServerCtx {
    pub panes: SharedPanes,
    pub cmd_tx: Sender<Command>,
    /// 앱 레벨 동작(브라우저/SFTP 탭). NabiApp::update가 drain.
    pub app_tx: Sender<nabi_proto::AppCtl>,
    pub policy: ControlPolicy,
    pub cfg: SpawnCfg,
    /// 이벤트 탭(Wait/Tail 구독용) — 오케스트레이터 이벤트 fan-out.
    pub events: crate::subscribe::EventHub,
}

/// 서버 스레드를 띄운다(실패는 로그만 — 제어 평면은 옵션 기능).
pub fn start(pipe: String, token: String, ctx: ServerCtx) {
    let _ = std::thread::Builder::new().name("nabi-control".into()).spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!(target: "control", %e, "제어 런타임 생성 실패");
                return;
            }
        };
        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, serve(pipe, token, ctx));
    });
}

/// 수락 루프: 인스턴스를 만들고 접속을 기다렸다가 연결별 태스크로 넘긴다.
/// 파이프는 현재 사용자 SID 전용 DACL로 만든다(CP-5 G5; 실패 시 기본 기술자 폴백).
async fn serve(pipe: String, token: String, ctx: ServerCtx) {
    use tokio::net::windows::named_pipe::ServerOptions;
    tracing::info!(target: "control", %pipe, "제어 서버 시작");
    let mut sec = crate::pipe_acl::PipeSecurity::current_user_only();
    if sec.is_none() {
        tracing::warn!(target: "control", "사용자 전용 DACL 생성 실패 — 기본 보안 기술자 사용");
    }
    loop {
        let made = match sec.as_mut() {
            Some(s) => unsafe {
                ServerOptions::new().create_with_security_attributes_raw(&pipe, s.attrs_ptr())
            },
            None => ServerOptions::new().create(&pipe),
        };
        let server = match made {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(target: "control", %e, "파이프 생성 실패 — 서버 중단");
                return;
            }
        };
        if server.connect().await.is_err() {
            continue;
        }
        let (t, c) = (token.clone(), ctx.clone());
        tokio::task::spawn_local(async move {
            handle_conn(server, t, c).await;
        });
    }
}

/// 한 연결: 첫 줄 Hello 토큰 검증 → 이후 줄마다 요청/응답.
async fn handle_conn(
    stream: tokio::net::windows::named_pipe::NamedPipeServer,
    token: String,
    ctx: ServerCtx,
) {
    let (r, mut w) = tokio::io::split(stream);
    let mut lines = BufReader::new(r).lines();
    // 인증: 첫 줄은 Hello여야 하고 토큰이 일치해야 한다.
    let from = match lines.next_line().await {
        Ok(Some(first)) => match serde_json::from_str::<ControlRequest>(&first) {
            Ok(ControlRequest::Hello { token: t, from }) if t == token => {
                let _ = write_resp(&mut w, &ControlResponse::Ok).await;
                from
            }
            _ => {
                tracing::warn!(target: "control", "인증 실패 접속 거부");
                let _ = write_resp(&mut w, &err("인증 실패")).await;
                return;
            }
        },
        _ => return,
    };
    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<ControlRequest>(&line) {
            // Wait/Tail은 읽기 전용 구독(read 그룹) — capture와 동급, 승인 불필요(G4).
            Ok(ControlRequest::Wait { pane, until, timeout_ms }) => {
                let cond = crate::subscribe::WaitCond::parse(&until);
                let resp =
                    crate::subscribe::run_wait(&ctx.events, &ctx.panes, pane, cond, timeout_ms).await;
                if write_resp(&mut w, &resp).await.is_err() {
                    return;
                }
            }
            Ok(ControlRequest::Tail { pane }) => {
                if stream_tail(&mut w, &ctx, pane).await.is_err() {
                    return;
                }
            }
            Ok(req) => {
                let resp = crate::dispatch::dispatch(
                    req, &ctx.panes, &ctx.cmd_tx, &ctx.app_tx, &ctx.policy, &ctx.cfg,
                    &ctx.events, from,
                );
                if write_resp(&mut w, &resp).await.is_err() {
                    return;
                }
            }
            Err(e) => {
                let _ = write_resp(&mut w, &err(&format!("요청 파싱 실패: {e}"))).await;
            }
        }
    }
}

/// Tail: 신규 줄만 델타 스트림(CP-5 G2 — 중복 재덤프·빠른 스크롤 유실 제거).
/// pane 종료 시 "exit" 이벤트를 보내고 스트림을 닫는다.
async fn stream_tail(
    w: &mut (impl AsyncWriteExt + Unpin),
    ctx: &ServerCtx,
    pane: u64,
) -> std::io::Result<()> {
    use nabi_proto::Event;
    use nabi_types::PaneId;
    let Some(model) = ctx
        .panes
        .read()
        .ok()
        .and_then(|m| m.get(&PaneId::new(pane)).cloned())
        .map(|v| v.model)
    else {
        return write_resp(w, &err(&format!("pane {pane} 없음"))).await;
    };
    // 접속 시점 이후의 신규 줄만 스트림.
    let mut last = model.lock().map(|m| m.line_marker()).unwrap_or(0);
    let rx = ctx.events.subscribe();
    loop {
        let mut changed = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, Event::PaneOutput { pane: p } if p.get() == pane) {
                changed = true;
            }
            if matches!(ev, Event::PaneExited { pane: p, .. } if p.get() == pane) {
                let bye = ControlResponse::Event { pane, kind: "exit".into(), data: String::new() };
                write_resp(w, &bye).await?;
                return Ok(());
            }
        }
        if changed {
            let delta = model.lock().ok().and_then(|m| crate::tail_delta::delta(&m, &mut last));
            if let Some(text) = delta {
                let resp = ControlResponse::Event { pane, kind: "output".into(), data: text };
                write_resp(w, &resp).await?;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }
}

async fn write_resp(
    w: &mut (impl AsyncWriteExt + Unpin),
    resp: &ControlResponse,
) -> std::io::Result<()> {
    let mut s = serde_json::to_string(resp).unwrap_or_else(|_| r#"{"res":"err"}"#.into());
    s.push('\n');
    w.write_all(s.as_bytes()).await
}

fn err(m: &str) -> ControlResponse {
    ControlResponse::Err { message: m.to_string() }
}
