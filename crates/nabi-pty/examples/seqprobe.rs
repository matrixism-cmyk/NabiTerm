//! 어떤 프로그램이 **화면을 지우는 신호를 보내는지** 직접 잡아 본다(진단용 도구).
//!
//! 스크롤백이 사라진다는 보고를 받았을 때, 원인을 짐작하지 말고 이걸로 확인한다.
//! 프로그램을 진짜 PTY 에서 띄우고 처음 몇 초 동안 나오는 바이트를 그대로 받아,
//! 스크롤백을 없애는 신호가 들어 있는지 센다.
//!
//! ```text
//! cargo run -p nabi-pty --example seqprobe -- claude
//! ```
//!
//! 보는 신호는 셋이다.
//!
//! * `ESC[3J` — **스크롤백을 통째로 지운다.** 우리 터미널 코어(alacritty)는 이 신호를 받으면
//!   지나간 내용을 전부 버린다. 이것이 나오면 원인이 확정된다.
//! * `ESC[?1049h` — 대체 화면으로 바꾼다. 대체 화면에서는 원래 스크롤백을 볼 수 없다(정상).
//! * `ESC[2J` — 보이는 화면만 지운다. 스크롤백은 남으므로 이것만으로는 문제가 아니다.

use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn main() {
    // 첫 인자가 프로그램, 그다음 `--` 뒤가 그 프로그램에 줄 인자, 마지막 숫자가 초.
    let all: Vec<String> = std::env::args().skip(1).collect();
    let secs: u64 = all.last().and_then(|s| s.parse().ok()).unwrap_or(6);
    let head: Vec<&String> = all.iter().filter(|a| a.parse::<u64>().is_err()).collect();
    let prog = head.first().map(|s| s.as_str()).unwrap_or("claude").to_string();
    let extra: Vec<String> = head.iter().skip(1).map(|s| (*s).clone()).collect();

    let pty = portable_pty::native_pty_system();
    let pair = pty
        .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
        .expect("PTY 를 열지 못했다");
    let mut cmd = CommandBuilder::new(&prog);
    for a in &extra {
        cmd.arg(a);
    }
    cmd.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(cmd).expect("프로그램을 띄우지 못했다");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("읽기 통로를 얻지 못했다");
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    // ConPTY 는 시작하면서 **커서 위치를 묻고 답을 기다린다**(ESC[6n). 답하지 않으면
    // 프로그램이 그 자리에서 멈춰 아무것도 내보내지 않는다 — 실제로 그렇게 헤맸다.
    let mut writer = pair.master.take_writer().expect("쓰기 통로를 얻지 못했다");
    let mut all = Vec::new();
    let mut answered = 0usize;
    // 프로그램이 뜨기를 기다렸다가 한마디 쳐 넣는다. 가만히 두면 시작 화면만 보고 끝나서
    // **정작 문제가 되는 다시 그리기를 한 번도 못 본다** — 그래서 실제로 일을 시킨다.
    let typed = std::env::var("NABI_PROBE_INPUT").unwrap_or_default();
    let mut type_at = (!typed.is_empty()).then(|| Instant::now() + Duration::from_secs(6));
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if type_at.is_some_and(|t| Instant::now() >= t) {
            type_at = None;
            let _ = writer.write_all(typed.as_bytes());
            let _ = writer.write_all(b"\r");
            let _ = writer.flush();
            println!("입력함: {typed:?}");
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => {
                all.extend_from_slice(&chunk);
                let asks = count(&chunk, b"\x1b[6n");
                for _ in 0..asks {
                    let _ = writer.write_all(b"\x1b[1;1R");
                    answered += 1;
                }
                let _ = writer.flush();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    if answered > 0 {
        println!("커서 위치 물음에 {answered}번 답했다");
    }
    let _ = child.kill();
    drop(pair.master);

    println!("프로그램: {prog} {:?} · 받은 바이트: {}", extra, all.len());
    // 아무것도 안 나오면 프로그램이 뜨지 않은 것이다 — 앞부분을 그대로 보여 준다.
    if all.len() < 64 {
        println!("  받은 것 전부: {:?}", String::from_utf8_lossy(&all));
    }
    for (label, pat, meaning) in [
        ("ESC[3J", &b"\x1b[3J"[..], "스크롤백을 통째로 지운다 (문제의 원인)"),
        ("ESC[?1049h", &b"\x1b[?1049h"[..], "대체 화면으로 바꾼다 (스크롤백을 못 봄)"),
        ("ESC[?1049l", &b"\x1b[?1049l"[..], "대체 화면에서 돌아온다"),
        ("ESC[2J", &b"\x1b[2J"[..], "보이는 화면만 지운다 (스크롤백은 남는다)"),
        ("ESC[H", &b"\x1b[H"[..], "커서를 맨 위로"),
        ("ESC[J", &b"\x1b[J"[..], "커서 아래를 지운다(ED0) — 다시 그리기의 기본"),
        ("ESC[0J", &b"\x1b[0J"[..], "위와 같다(번호를 붙인 형태)"),
        ("ESC[1J", &b"\x1b[1J"[..], "커서 위를 지운다(ED1)"),
        ("ESC[?1049", &b"\x1b[?1049"[..], "대체 화면 전환(양쪽 합)"),
    ] {
        println!("  {:<12} {:>5}번  {meaning}", label, count(&all, pat));
    }
}

/// 겹치지 않게 센다.
fn count(hay: &[u8], needle: &[u8]) -> usize {
    let (mut i, mut n) = (0usize, 0usize);
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            n += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    n
}
