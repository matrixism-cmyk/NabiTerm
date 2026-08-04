//! 오케스트레이터 액터 루프(스레드).

use crate::commands::handle_command;
use crate::handle::OrchestratorHandle;
use crate::pane_registry::{new_shared_panes, PaneRuntime};
use crate::router::route_output;
use bytes::Bytes;
use crossbeam_channel::unbounded;
use nabi_log::pane_span;
use nabi_proto::{Command, Event};
use nabi_types::PaneId;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// catch_unwind가 잡은 패닉 페이로드에서 사람이 읽을 메시지를 뽑는다.
fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "알 수 없는 패닉".to_string()
    }
}

/// 오케스트레이터 스레드를 띄우고 UI 핸들을 돌려준다.
///
/// `waker`는 메시지(특히 pane 출력)를 처리할 때마다 호출돼 UI 스레드를 깨운다 — 없으면 출력/
/// PTY 에코가 다음 유휴 하트비트까지 화면에 안 나타나 입력이 느리게 느껴진다(reactive 모드).
pub fn start<W: Fn() + Send + 'static>(waker: W) -> OrchestratorHandle {
    let (cmd_tx, cmd_rx) = unbounded::<Command>();
    let (event_tx, event_rx) = unbounded::<Event>();
    let (out_tx, out_rx) = unbounded::<(PaneId, Bytes)>();
    let panes = new_shared_panes();
    let panes_thread = panes.clone();

    let _ = std::thread::Builder::new()
        .name("orchestrator".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio 런타임 생성 실패");
            let mut state: HashMap<PaneId, PaneRuntime> = HashMap::new();
            let mut sftp = crate::sftp::SftpConns::new();
            let mut forwards: HashMap<u64, tokio::task::JoinHandle<()>> = HashMap::new();
            // 병렬 ConPTY 스폰: 완료를 seq 순서로 방출(UI의 origin/백로그 FIFO 보존).
            let (done_tx, done_rx) = unbounded::<crate::spawn_pane::SpawnDone>();
            // 셸 자연 종료 통지(waiter 스레드 → 액터): 레지스트리 정리 + PaneExited.
            let (exit_tx, exit_rx) = unbounded::<(PaneId, Option<i32>)>();
            let (mut seq_alloc, mut seq_emit) = (0u64, 0u64);
            let mut pending_done: std::collections::BTreeMap<u64, crate::spawn_pane::SpawnDone> =
                std::collections::BTreeMap::new();
            let mut inflight: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
            let mut closed_early: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
            let mut stash: HashMap<PaneId, Vec<Bytes>> = HashMap::new(); // 등록 전 도착 출력.
            // 호스트키 사용자 확인기(미지 키 → UI 모달 → 결정 회신).
            let verifier = crate::hostkey::OrchVerifier::new(event_tx.clone());
            tracing::info!("오케스트레이터 시작");
            loop {
                crossbeam_channel::select! {
                    recv(cmd_rx) -> msg => match msg {
                        Ok(Command::Shutdown) | Err(_) => break,
                        // 로컬 셸 스폰은 블로킹(ConPTY 생성)이라 스레드로 — 복원 시 병렬화.
                        Ok(Command::SpawnLocalPane { shell, size, scrollback, encoding, cwd, reply_seq }) => {
                            let p = crate::spawn_pane::begin_local_spawn(
                                shell, size, scrollback, encoding, cwd, reply_seq,
                                seq_alloc, out_tx.clone(), done_tx.clone(), exit_tx.clone(),
                            );
                            inflight.insert(p);
                            seq_alloc += 1;
                        }
                        // 명령 처리 패닉을 격리: 한 명령이 죽어도 오케스트레이터 스레드는 살린다.
                        Ok(cmd) => {
                            // 스폰 진행 중 pane이 닫히면 완료 시 폐기하도록 표시.
                            if let Command::ClosePane { pane } = &cmd {
                                if inflight.contains(pane) {
                                    closed_early.insert(*pane);
                                }
                            }
                            let r = catch_unwind(AssertUnwindSafe(|| handle_command(
                                cmd, rt.handle(), &mut state, &mut sftp, &mut forwards,
                                &panes_thread, &out_tx, &event_tx, &verifier,
                            )));
                            if let Err(e) = r {
                                let info = panic_text(e);
                                tracing::error!(panic = %info, "명령 처리 패닉(격리됨)");
                                let _ = event_tx.send(Event::Error {
                                    message: format!("명령 처리 오류(격리됨): {info}"),
                                });
                            }
                        }
                    },
                    recv(exit_rx) -> msg => {
                        if let Ok((pane, code)) = msg {
                            // 셸 종료: 레지스트리에서 제거(좀비·핸들 누수 방지) + UI 통지.
                            state.remove(&pane);
                            panes_thread.write().unwrap().remove(&pane);
                            let _ = event_tx.send(Event::PaneExited { pane, code });
                        }
                    },
                    recv(done_rx) -> msg => {
                        if let Ok(d) = msg {
                            pending_done.insert(d.seq, d);
                            // 명령 순서(seq)대로만 등록·방출 — 중간이 비면 대기.
                            while let Some(d) = pending_done.remove(&seq_emit) {
                                seq_emit += 1;
                                inflight.remove(&d.pane);
                                let pane = d.pane;
                                if closed_early.remove(&pane) {
                                    stash.remove(&pane);
                                    continue; // 스폰 중 닫힘 — pty 드롭(자식 종료).
                                }
                                crate::spawn_pane::finish_local_spawn(d, &mut state, &panes_thread, &event_tx);
                                // 등록 전 도착했던 출력 재생(셸 배너 유실 방지).
                                for b in stash.remove(&pane).unwrap_or_default() {
                                    route_output(pane, &b, &mut state, &panes_thread, &event_tx);
                                }
                            }
                        }
                    },
                    recv(out_rx) -> msg => {
                        if let Ok((pane, bytes)) = msg {
                            // 아직 등록 전(스폰 진행 중)이면 보관했다가 등록 후 재생.
                            if inflight.contains(&pane) {
                                stash.entry(pane).or_default().push(bytes);
                                continue;
                            }
                            // pane span으로 로그를 pane별로 구분 + 출력 처리 패닉 격리.
                            let _span = pane_span(pane).entered();
                            let r = catch_unwind(AssertUnwindSafe(|| {
                                route_output(pane, &bytes, &mut state, &panes_thread, &event_tx);
                            }));
                            if let Err(e) = r {
                                let info = panic_text(e);
                                tracing::error!(panic = %info, "출력 처리 패닉(격리됨) — 해당 pane 중단");
                                let _ = event_tx.send(Event::Error {
                                    message: format!("pane {} 출력 처리 오류(격리됨)", pane.get()),
                                });
                            }
                        }
                    }
                }
                waker(); // 메시지 처리(출력/스폰/종료 등) 후 UI 즉시 깨우기 — 입력/출력 지연 해소.
            }
            tracing::info!("오케스트레이터 종료");
        });

    OrchestratorHandle {
        cmd_tx,
        event_rx,
        panes,
    }
}
