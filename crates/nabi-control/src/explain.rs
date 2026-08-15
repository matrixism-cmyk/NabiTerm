//! `agent explain`(A4) — 화면을 감지 규칙으로 평가한 근거 회신. dispatch.rs에서 분리(라인 한도).

use crate::protocol::ControlResponse;
use nabi_orchestrator::SharedPanes;
use nabi_types::PaneId;

fn err(m: &str) -> ControlResponse {
    ControlResponse::Err { message: m.into() }
}

/// pane 하단 화면을 내장 감지 규칙(claude/codex/…) 전부로 평가해 근거를 회신한다.
///
/// 어떤 매니페스트가 무엇에 매치했는지 그대로 보여 준다 — 사용자 오버라이드 규칙을 만들 때
/// "왜 이 상태로 판정됐나"를 추측하지 않게. (앱 내 감지는 실행 명령으로 매니페스트를
/// 고르지만, 여기는 디버깅용이라 전부 평가한다. 사용자 폴더 규칙은 앱 프로세스 소관 — 내장만.)
pub(crate) fn agent_explain(panes: &SharedPanes, pane: u64) -> ControlResponse {
    let Some(view) = panes.read().ok().and_then(|m| m.get(&PaneId::new(pane)).cloned()) else {
        return err("pane 없음");
    };
    let (bottom, title) = match view.model.lock() {
        Ok(md) => (md.visible_bottom_text(4), view.title.clone()),
        Err(_) => return err("모델 잠금 실패"),
    };
    let screen = nabi_agentdetect::Screen { bottom: &bottom, title: &title };
    let results: Vec<_> = nabi_agentdetect::builtin()
        .iter()
        .map(|m| {
            let (state, rule) = nabi_agentdetect::classify(m, &screen);
            serde_json::json!({ "manifest": m.id, "state": format!("{state:?}"), "rule": rule })
        })
        .collect();
    ControlResponse::Event {
        pane,
        kind: "agent-explain".into(),
        data: serde_json::json!({ "bottom": bottom, "title": title, "results": results })
            .to_string(),
    }
}
