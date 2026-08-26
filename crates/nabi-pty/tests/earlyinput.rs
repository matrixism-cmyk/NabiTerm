//! **스폰 직후 써 넣은 입력이 정말 실행되는가** — 워크스페이스 복원의 전제.
//!
//! 복원은 pane을 띄운 **그 즉시** `on_connect` 명령을 PTY에 써 넣는다. 그런데 그때
//! PowerShell은 아직 시작도 못 했다(프로필 실행 전). ConPTY 입력 파이프에 들어간 바이트가
//! 셸이 읽기를 시작할 때까지 살아남는지는 **가정이지 사실이 아니었다.**
//!
//! 사용자 보고(2026-08-26): "claude를 켜 둔 채 종료했다 켜면 pane은 열리는데 claude가
//! 다시 뜨지 않는다." 저장 파일에는 명령이 옵션까지 멀쩡히 들어 있었으므로 남는 곳은
//! 여기뿐이다.
//!
//! 실 셸을 띄우므로 `--ignored`. 실행:
//! `cargo test -p nabi-pty --test earlyinput -- --ignored --nocapture`

use bytes::Bytes;
use nabi_proto::ShellKind;
use nabi_types::{GridSize, PaneId};
use std::time::{Duration, Instant};

/// 셸을 띄우고 `delay` 뒤에 명령을 써 넣은 다음, 표식이 출력에 나타나는지 본다.
fn ran_after(delay: Duration) -> (bool, String) {
    let (tx, rx) = crossbeam_channel::unbounded::<(PaneId, Bytes)>();
    let pane = PaneId::new(1);
    let mut pty = nabi_pty::spawn_local(
        pane,
        &ShellKind::WindowsPowerShell,
        GridSize::new(100, 30),
        tx,
        None,
        Box::new(|_| {}),
    )
    .expect("셸 스폰 실패");
    std::thread::sleep(delay);
    use nabi_pty::ByteChannel;
    pty.write(b"echo NABIMARK$(1+1)\r").expect("입력 쓰기 실패");

    let mut out = String::new();
    let until = Instant::now() + Duration::from_secs(20);
    while Instant::now() < until {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok((_, b)) => {
                out.push_str(&String::from_utf8_lossy(&b));
                // 에코가 아니라 **실행 결과**를 본다(`$(1+1)`이 2로 펼쳐진 형태).
                if out.contains("NABIMARK2") {
                    return (true, out);
                }
            }
            Err(_) => continue,
        }
    }
    (false, out)
}

/// 복원이 실제로 하는 일 — 스폰 직후(한 프레임 남짓) 써 넣기.
#[test]
#[ignore = "실 PowerShell을 띄운다"]
fn input_written_right_after_spawn_actually_runs() {
    let (ok, out) = ran_after(Duration::from_millis(20));
    println!("--- 20ms 뒤 쓰기: {}\n{}", if ok { "실행됨" } else { "삼켜짐" }, tail(&out));
    assert!(ok, "스폰 직후 써 넣은 명령이 실행되지 않았다 — 복원이 여기서 깨진다");
}

/// 비교군 — 셸이 프롬프트를 낼 시간을 준 뒤 쓰면 되는가.
#[test]
#[ignore = "실 PowerShell을 띄운다"]
fn input_written_after_the_shell_settles_runs() {
    let (ok, out) = ran_after(Duration::from_millis(2500));
    println!("--- 2.5s 뒤 쓰기: {}\n{}", if ok { "실행됨" } else { "삼켜짐" }, tail(&out));
    assert!(ok, "셸이 준비된 뒤에도 입력이 먹지 않는다 — 전혀 다른 문제다");
}

fn tail(s: &str) -> String {
    let n = s.chars().count();
    s.chars().skip(n.saturating_sub(600)).collect()
}
