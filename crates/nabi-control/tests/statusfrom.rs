//! **누가 보냈는지가 끝까지 남는가** — 상태가 부른 pane 에 꽂히는지 본다.
//!
//! ## 왜 따로 있는가
//!
//! 이 시험은 `NABI_PANE_ID` 환경 변수를 건드린다. 환경 변수는 프로세스 하나에 하나뿐이라
//! 같은 파일 안의 다른 시험과 같이 돌면 서로 덮는다. 그래서 **파일을 따로 둔다** —
//! 카고는 시험 파일마다 실행 파일을 따로 만들므로, 여기 있는 것만 이 변수를 본다.
//!
//! ## 무엇을 막는가
//!
//! `server.rs` 가 부른 쪽의 pane 번호를 버리던 때가 있었다. 그러면 상태가 pane 0 에 쌓이는데
//! pane 번호는 1부터라 **0 은 존재한 적이 없다.** `status set`·`agent report`·`agent session`
//! 셋이 성공을 돌려주면서 아무 일도 하지 않았고, 그래서 **작업 공간을 되살릴 때 대화 번호를
//! 못 찾아 매번 처음부터 시작했다.**
//!
//! 왕복 시험은 "오류가 아니다"만 봤기 때문에 그때도 초록이었다. 여기서는 **어느 pane 에
//! 꽂혔는지**를 본다 — 그것이 어긋난 바로 그 값이다.

use crossbeam_channel::unbounded;
use nabi_control::protocol::{ControlRequest, ControlResponse};
use nabi_orchestrator::pane_registry::new_shared_panes;
use nabi_orchestrator::PaneView;
use nabi_proto::AppCtl;
use nabi_types::{GridSize, PaneId};
use std::sync::{Arc, Mutex};

/// 부르는 쪽이 pane 7 이라고 밝히면 상태도 pane 7 에 간다.
#[test]
fn status_lands_on_the_pane_that_asked() {
    let pipe = format!(r"\\.\pipe\nabi-ctl-from-{}", std::process::id());
    let token = nabi_control::gen_token();
    let panes = new_shared_panes();
    let model = nabi_vt::TermModel::new(GridSize::new(40, 5), 100);
    panes.write().unwrap().insert(
        PaneId::new(7),
        PaneView::new(Arc::new(Mutex::new(model)), "테스트셸".into(), "local"),
    );
    let (cmd_tx, _cmd_rx) = unbounded();
    let (app_tx, app_rx) = unbounded();
    let (policy, _ask_rx) = nabi_control::policy::ControlPolicy::new(nabi_control::policy::Mode::On);
    nabi_control::server::start(
        pipe.clone(),
        token.clone(),
        nabi_control::server::ServerCtx {
            panes,
            cmd_tx,
            app_tx,
            policy,
            cfg: nabi_control::dispatch::SpawnCfg {
                scrollback: 100,
                encoding: "UTF-8".into(),
                cols: 80,
                rows: 24,
            },
            events: nabi_control::subscribe::EventHub::new(),
        },
    );

    // pane 밖에서 부르면 **조용히 사라지지 않고** 그렇다고 말한다.
    std::env::remove_var("NABI_PANE_ID");
    let r = nabi_control::client::request(&pipe, &token, &status_req()).unwrap();
    assert!(
        matches!(r, ControlResponse::Err { .. }),
        "pane 을 모르는 채로 상태를 적으면 오류여야 한다: {r:?}"
    );
    assert!(
        app_rx.try_recv().is_err(),
        "적을 곳이 없는데 앱에 무언가를 보냈다 — 없는 pane 에 쌓이던 그 버그다"
    );

    // pane 7 이 부르면 pane 7 에 꽂힌다.
    std::env::set_var("NABI_PANE_ID", "7");
    let r = nabi_control::client::request(&pipe, &token, &status_req()).unwrap();
    assert!(matches!(r, ControlResponse::Ok), "상태 설정이 거절됐다: {r:?}");
    match app_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap() {
        AppCtl::PaneStatus { pane, key, value, .. } => {
            // 0 이 나오면 부른 쪽 번호를 버린 것이다(pane 번호는 1부터다).
            assert_eq!(pane, 7, "상태가 엉뚱한 pane 에 갔다");
            assert_eq!(key, "state");
            assert_eq!(value.as_deref(), Some("working"));
        }
        other => panic!("PaneStatus 가 아님: {other:?}"),
    }
    std::env::remove_var("NABI_PANE_ID");
}

fn status_req() -> ControlRequest {
    ControlRequest::PaneStatusSet {
        key: "state".into(),
        value: Some("working".into()),
        ttl_ms: None,
    }
}
