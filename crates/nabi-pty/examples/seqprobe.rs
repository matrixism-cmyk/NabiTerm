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
use std::io::Read;
use std::time::{Duration, Instant};

fn main() {
    let prog = std::env::args().nth(1).unwrap_or_else(|| "claude".into());
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(6);

    let pty = portable_pty::native_pty_system();
    let pair = pty
        .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
        .expect("PTY 를 열지 못했다");
    let mut cmd = CommandBuilder::new(&prog);
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

    let mut all = Vec::new();
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => all.extend_from_slice(&chunk),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    let _ = child.kill();
    drop(pair.master);

    println!("프로그램: {prog} · 받은 바이트: {}", all.len());
    for (label, pat, meaning) in [
        ("ESC[3J", &b"\x1b[3J"[..], "스크롤백을 통째로 지운다 (문제의 원인)"),
        ("ESC[?1049h", &b"\x1b[?1049h"[..], "대체 화면으로 바꾼다 (스크롤백을 못 봄)"),
        ("ESC[?1049l", &b"\x1b[?1049l"[..], "대체 화면에서 돌아온다"),
        ("ESC[2J", &b"\x1b[2J"[..], "보이는 화면만 지운다 (스크롤백은 남는다)"),
        ("ESC[H", &b"\x1b[H"[..], "커서를 맨 위로"),
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
