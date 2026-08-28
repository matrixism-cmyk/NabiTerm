//! 쓰기 동작의 관문 — 정책을 묻고 **자취를 남긴다**(배치 AB).
//!
//! `dispatch.rs`에서 떼어냈다(라인 한도). 떼어 놓으니 경계도 분명해진다 — 여기는
//! "해도 되는가"와 "무엇을 했는가"만 다루고, 실제 수행은 `dispatch_write`가 한다.

use crate::dispatch::{dispatch_write, group_of, SpawnCfg};
use crate::dispatchread::err;
use crate::policy::ControlPolicy;
use crate::protocol::{ControlRequest, ControlResponse};
use crate::subscribe::EventHub;
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::{AppCtl, Command};

/// 쓰기 동작의 관문 — 정책을 묻고, **무엇을 했든 자취를 남긴다.**
///
/// 감사 기록이 여기 한 곳이면 되는 이유: 쓰기 동작이 전부 이 갈래를 지난다. 동사마다 따로
/// 적으면 새 동사를 더할 때 빠뜨리고, 빠진 것은 조용히 는다(지금까지 22개 중 18개만
/// `tracing` 이 있었던 것이 그 결과다).
#[allow(clippy::too_many_arguments)]
pub(crate) fn gated_write(
    req: ControlRequest,
    panes: &SharedPanes,
    cmd_tx: &Sender<Command>,
    app_tx: &Sender<AppCtl>,
    policy: &ControlPolicy,
    cfg: &SpawnCfg,
    events: &EventHub,
    from: Option<u64>,
) -> ControlResponse {
    let g = group_of(&req);
    let (verb, target, bytes) = crate::trail::describe(&req);
    if !policy.allow(g, from) {
        tracing::warn!(target: "control", from = ?from, group = ?g, "거부(정책)");
        // **거부도 남긴다.** 무엇을 시도했는지가 감사의 절반이다.
        crate::trail::note(from, verb, target, bytes, crate::trail::Outcome::Denied);
        let msg = match policy.mode() {
            crate::policy::Mode::Off => "제어가 꺼져 있음(설정 control.mode)",
            _ => "승인 대기 — nabiTerm에서 승인 후 다시 시도하세요",
        };
        return err(msg);
    }
    let resp = dispatch_write(req, panes, cmd_tx, app_tx, cfg, events, from);
    let outcome = match &resp {
        ControlResponse::Err { .. } => crate::trail::Outcome::Failed,
        _ => crate::trail::Outcome::Allowed,
    };
    crate::trail::note(from, verb, target, bytes, outcome);
    resp
}
