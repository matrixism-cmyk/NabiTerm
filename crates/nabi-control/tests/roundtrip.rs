//! 제어 평면 왕복 검증: 실제 named pipe로 list/capture + 인증 실패 거부.

use crossbeam_channel::unbounded;
use nabi_control::protocol::{ControlRequest, ControlResponse};
use nabi_orchestrator::pane_registry::new_shared_panes;
use nabi_orchestrator::PaneView;
use nabi_types::{GridSize, PaneId};
use std::sync::{Arc, Mutex};

#[test]
fn pipe_roundtrip_list_capture_and_auth() {
    let pipe = format!(r"\\.\pipe\nabi-ctl-test-{}", std::process::id());
    let token = nabi_control::gen_token();
    let panes = new_shared_panes();
    // 가짜 pane: 화면에 "hello control" 출력.
    let mut model = nabi_vt::TermModel::new(GridSize::new(40, 5), 100);
    model.process(b"hello control\r\n");
    panes.write().unwrap().insert(
        PaneId::new(7),
        PaneView::new(Arc::new(Mutex::new(model)), "테스트셸".into(), "local"),
    );
    let (cmd_tx, cmd_rx) = unbounded();
    let (app_tx, _app_rx) = unbounded();
    let (policy, _ask_rx) = nabi_control::policy::ControlPolicy::new(nabi_control::policy::Mode::On);
    let ctx = nabi_control::server::ServerCtx {
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
    };
    nabi_control::server::start(pipe.clone(), token.clone(), ctx);
    // 클라이언트가 자체적으로 파이프 준비/혼잡을 재시도한다.

    // list: pane 7이 보인다.
    let r = nabi_control::client::request(&pipe, &token, &ControlRequest::ListPanes).unwrap();
    match r {
        ControlResponse::Panes { panes } => {
            assert_eq!(panes.len(), 1);
            assert_eq!(panes[0].id, 7);
            assert_eq!((panes[0].cols, panes[0].rows), (40, 5));
        }
        other => panic!("Panes 응답이 아님: {other:?}"),
    }
    // capture: 출력 텍스트가 돌아온다.
    let r = nabi_control::client::request(
        &pipe,
        &token,
        &ControlRequest::Capture { pane: 7, lines: 10, start: None, end: None, escapes: false },
    )
    .unwrap();
    match r {
        ControlResponse::Captured { text, .. } => assert!(text.contains("hello control")),
        other => panic!("Captured 응답이 아님: {other:?}"),
    }
    // 쓰기 동작(send): On 모드라 허용 → cmd_tx로 WriteInput 방출.
    let r = nabi_control::client::request(
        &pipe,
        &token,
        &ControlRequest::SendInput { pane: 7, data: "echo hi\r".into(), raw: false },
    )
    .unwrap();
    assert!(matches!(r, ControlResponse::Ok));
    match cmd_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap() {
        nabi_proto::Command::WriteInput { pane, data } => {
            assert_eq!(pane.get(), 7);
            // 타깃이 bracketed paste 미사용 → 원문 그대로.
            assert_eq!(&data[..], b"echo hi\r");
        }
        other => panic!("WriteInput 명령이 아님: {other:?}"),
    }
    // 잘못된 토큰은 인증 단계에서 거부.
    let bad = nabi_control::client::request(&pipe, "wrong", &ControlRequest::ListPanes);
    assert!(bad.is_err());
}

/// G1: 스폰 응답이 seq로 상관된다 — 다른 스폰의 PaneSpawned(엉뚱한 seq)가
/// 먼저 와도 자기 seq의 pane ID만 회신(예전 before/after 폴링은 999를 집었음).
#[test]
fn spawn_returns_seq_matched_pane() {
    let pipe = format!(r"\\.\pipe\nabi-ctl-spawn-{}", std::process::id());
    let token = nabi_control::gen_token();
    let (cmd_tx, cmd_rx) = unbounded();
    let (app_tx, _app_rx) = unbounded();
    let (policy, _ask_rx) = nabi_control::policy::ControlPolicy::new(nabi_control::policy::Mode::On);
    let events = nabi_control::subscribe::EventHub::new();
    let ctx = nabi_control::server::ServerCtx {
        panes: new_shared_panes(),
        cmd_tx,
        app_tx,
        policy,
        cfg: nabi_control::dispatch::SpawnCfg {
            scrollback: 100,
            encoding: "UTF-8".into(),
            cols: 80,
            rows: 24,
        },
        events: events.clone(),
    };
    nabi_control::server::start(pipe.clone(), token.clone(), ctx);
    // 가짜 액터: 스폰 명령을 받으면 엉뚱한 seq 먼저, 그 다음 올바른 seq로 응답.
    std::thread::spawn(move || {
        if let Ok(nabi_proto::Command::SpawnLocalPane { reply_seq: Some(s), .. }) =
            cmd_rx.recv_timeout(std::time::Duration::from_secs(5))
        {
            events.publish(&nabi_proto::Event::PaneSpawned {
                pane: PaneId::new(999),
                seq: Some(s + 777),
            });
            events.publish(&nabi_proto::Event::PaneSpawned { pane: PaneId::new(42), seq: Some(s) });
        }
    });
    let r = nabi_control::client::request(
        &pipe,
        &token,
        &ControlRequest::SpawnTerminal { shell: "cmd".into(), cwd: None, dock: None, ssh: None },
    )
    .unwrap();
    assert!(matches!(r, ControlResponse::Spawned { pane: 42 }), "{r:?}");
}

/// B2: wait --until output --match 는 "아무 출력"이 아니라 패턴 등장을 기다린다.
#[test]
fn wait_output_match_finds_pattern_line() {
    let hub = nabi_control::subscribe::EventHub::new();
    let panes = new_shared_panes();
    let model = Arc::new(Mutex::new(nabi_vt::TermModel::new(GridSize::new(30, 4), 10)));
    panes.write().unwrap().insert(
        PaneId::new(9),
        PaneView::new(model.clone(), "t".into(), "local"),
    );
    let h2 = hub.clone();
    let m2 = model.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(80));
        // 관심 없는 출력 먼저 — 패턴 대기는 이걸로 충족되면 안 된다.
        m2.lock().unwrap().process(b"compiling...\r\n");
        h2.publish(&nabi_proto::Event::PaneOutput { pane: PaneId::new(9) });
        std::thread::sleep(std::time::Duration::from_millis(120));
        m2.lock().unwrap().process(b"BUILD OK 42\r\n");
        h2.publish(&nabi_proto::Event::PaneOutput { pane: PaneId::new(9) });
    });
    let pat = nabi_control::subscribe::Matcher::build(None, Some(r"BUILD OK \d+".into()))
        .unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let resp = rt.block_on(nabi_control::subscribe::run_wait(
        &hub,
        &panes,
        9,
        nabi_control::subscribe::WaitCond::Output,
        3000,
        pat,
    ));
    match resp {
        ControlResponse::Event { kind, data, .. } => {
            assert_eq!(kind, "output-match");
            assert!(data.contains("BUILD OK 42"), "{data}");
        }
        other => panic!("Event 응답이 아님: {other:?}"),
    }
    // 깨진 정규식은 만들 때 바로 거부된다(대기 걸고 나서 조용히 실패하지 않게).
    assert!(nabi_control::subscribe::Matcher::build(None, Some("([".into())).is_err());
}

/// G3: wait --until exit가 종료 코드를 JSON으로 회신한다.
#[test]
fn wait_exit_returns_code() {
    let hub = nabi_control::subscribe::EventHub::new();
    let panes = new_shared_panes();
    panes.write().unwrap().insert(
        PaneId::new(3),
        PaneView::new(
            Arc::new(Mutex::new(nabi_vt::TermModel::new(GridSize::new(10, 3), 10))),
            "t".into(),
            "local",
        ),
    );
    let h2 = hub.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        h2.publish(&nabi_proto::Event::PaneExited { pane: PaneId::new(3), code: Some(7) });
    });
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let resp = rt.block_on(nabi_control::subscribe::run_wait(
        &hub,
        &panes,
        3,
        nabi_control::subscribe::WaitCond::Exit,
        3000,
        None,
    ));
    match resp {
        ControlResponse::Event { kind, data, .. } => {
            assert_eq!(kind, "exit");
            assert!(data.contains("\"code\":7"), "{data}");
        }
        other => panic!("Event 응답이 아님: {other:?}"),
    }
}

#[test]
fn ask_mode_groups_and_approval() {
    use nabi_control::policy::{ControlPolicy, Group, Mode};
    let (policy, ask_rx) = ControlPolicy::new(Mode::Ask);
    // read 그룹은 ask 모드에서도 무승인(G4).
    assert!(policy.allow(Group::Read, Some(9)));
    // 미승인 inject: 거부 + (인스턴스=0, 그룹) 승인 요청 발생. 클라이언트가 주장한
    // pane ID는 인증된 신원이 아니므로 승인 키로 사용하지 않는다.
    assert!(!policy.allow(Group::Inject, Some(5)));
    assert_eq!(
        ask_rx.recv_timeout(std::time::Duration::from_millis(200)).unwrap(),
        (0, Group::Inject)
    );
    // act 승인이 inject를 풀지 않음(별도 집합 — CP-7).
    policy.approve(5, Group::Act);
    assert!(policy.allow(Group::Act, Some(5)));
    assert!(!policy.allow(Group::Inject, Some(5)));
    // inject 승인 후 허용, revoke로 회수.
    policy.approve(5, Group::Inject);
    assert!(policy.allow(Group::Inject, Some(5)));
    policy.revoke(5, Group::Inject);
    assert!(!policy.allow(Group::Inject, Some(5)));
    // revoke 뒤에는 어떤 주장 pane에서도 거부.
    assert!(!policy.allow(Group::Inject, Some(6)));
}
