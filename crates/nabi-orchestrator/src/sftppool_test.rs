//! 워커 풀 실서버 검증 — **정말 동시에** 전송되는지 본다.
//!
//! 인프로세스로는 증명이 안 된다. "동시"의 반대는 "빨리 끝난다"가 아니라 "겹친다"이므로,
//! 시간을 재는 대신 **첫 완료 전에 두 전송 모두에서 진행 이벤트가 나왔는지**를 본다.
//! 직렬이면 두 번째 전송의 진행 이벤트는 첫 완료 뒤에야 나온다.
//!
//! 실행: `NABI_RT_USER` `NABI_RT_PASS`(+ `NABI_RT_HOST`/`NABI_RT_PORT`) 후
//! `cargo test -p nabi-orchestrator -- --ignored --nocapture`.

use crate::sftppool::{new_flags, Job, Pool};
use nabi_proto::{Event, SshParams};

/// 전송 하나당 크기(진행 이벤트가 여러 번 나오도록 넉넉히).
const MB: usize = 12;

fn params() -> Option<SshParams> {
    let user = std::env::var("NABI_RT_USER").ok()?;
    let pass = std::env::var("NABI_RT_PASS").ok()?;
    let host = std::env::var("NABI_RT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("NABI_RT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(22);
    Some(SshParams::password(host, port, user, pass))
}

/// 테스트용 로컬 파일을 만들고 경로를 돌려준다.
fn make_file(tag: &str) -> String {
    let p = std::env::temp_dir().join(format!("nabi-pool-{}-{}.bin", std::process::id(), tag));
    let block = vec![b'x'; 1024 * 1024];
    let mut data = Vec::with_capacity(MB * block.len());
    for _ in 0..MB {
        data.extend_from_slice(&block);
    }
    std::fs::write(&p, &data).unwrap();
    p.to_string_lossy().into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "실 서버 필요(NABI_RT_USER/NABI_RT_PASS)"]
async fn two_uploads_actually_overlap() {
    let Some(p) = params() else { return };
    let (tx, rx) = crossbeam_channel::unbounded::<Event>();
    let mut pool = Pool::new(1, p, 0, 2, tx, new_flags());
    let (a, b) = (make_file("a"), make_file("b"));
    let (ra, rb) = ("nabi_pool_a.bin".to_string(), "nabi_pool_b.bin".to_string());
    pool.dispatch(Job::Upload { xfer: 1, local: a.clone(), remote: ra.clone() });
    pool.dispatch(Job::Upload { xfer: 2, local: b.clone(), remote: rb.clone() });

    let mut seen = (false, false);
    let mut overlapped = false;
    let mut done = 0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    while done < 2 && std::time::Instant::now() < deadline {
        let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs(60)) else { break };
        match e {
            Event::SftpProgress { xfer, .. } => {
                if xfer == 1 {
                    seen.0 = true;
                }
                if xfer == 2 {
                    seen.1 = true;
                }
                // 아직 아무것도 안 끝났는데 둘 다 진행 중 = 겹쳤다.
                if done == 0 && seen.0 && seen.1 {
                    overlapped = true;
                }
            }
            Event::SftpTransferDone { xfer, ok, message, .. } => {
                assert!(ok, "전송 {xfer} 실패: {message}");
                done += 1;
            }
            _ => {}
        }
    }
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    assert_eq!(done, 2, "두 전송 모두 끝나야 한다");
    assert!(overlapped, "직렬로 돌았다 — 워커 풀이 동시에 보내지 못하고 있다");
}

/// 하나만 취소하면 나머지는 끝까지 간다(예전에는 연결째로 끊어 둘 다 죽었다).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "실 서버 필요(NABI_RT_USER/NABI_RT_PASS)"]
async fn cancel_one_leaves_the_other_running() {
    let Some(p) = params() else { return };
    let (tx, rx) = crossbeam_channel::unbounded::<Event>();
    let flags = new_flags();
    // 속도를 묶는다. 안 그러면 로컬에서 12MB가 1초대에 끝나 취소가 도착하기 전에 완료된다
    // (실제로 그래서 처음엔 이 테스트가 헛돌았다). 2MB/s면 6초쯤 걸려 확실히 중간에 걸린다.
    let mut pool = Pool::new(1, p, 2000, 2, tx, flags.clone());
    let (a, b) = (make_file("c"), make_file("d"));
    pool.dispatch(Job::Upload { xfer: 1, local: a.clone(), remote: "nabi_pool_c.bin".into() });
    pool.dispatch(Job::Upload { xfer: 2, local: b.clone(), remote: "nabi_pool_d.bin".into() });

    let mut cancelled = false;
    let (mut ok2, mut fail1) = (false, false);
    let mut done = 0;
    while done < 2 {
        let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs(60)) else { break };
        match e {
            // 1번이 실제로 흐르기 시작한 뒤에 끊는다(시작 전에 끊으면 검증이 약해진다).
            Event::SftpProgress { xfer: 1, .. } if !cancelled => {
                crate::sftppool::cancel_one(&flags, 1);
                cancelled = true;
            }
            Event::SftpTransferDone { xfer, ok, .. } => {
                done += 1;
                if xfer == 1 && !ok {
                    fail1 = true;
                }
                if xfer == 2 && ok {
                    ok2 = true;
                }
            }
            _ => {}
        }
    }
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    assert!(cancelled, "취소를 걸 만큼 진행되지 않았다");
    assert!(fail1, "취소한 전송은 실패로 끝나야 한다");
    assert!(ok2, "취소하지 않은 전송은 끝까지 가야 한다");
}
