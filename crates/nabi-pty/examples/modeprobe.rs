//! TUI가 켜는 터미널 모드를 실제로 확인한다(휠 스크롤 진단용).
//!
//! `cargo run -p nabi-pty --example modeprobe -- <명령> [인자...]`
//! 앱을 PTY에 띄우고 첫 출력에서 DEC private mode 설정(`CSI ? n h/l`)만 추려 보고한다.
//! 화면에 뭐가 그려지는지가 아니라 **무엇을 켰는지**가 휠 동작을 가른다.

use std::io::Read;
use std::time::{Duration, Instant};

fn label(n: &str) -> &'static str {
    match n {
        "1049" | "47" | "1047" => "대체화면(alt screen)",
        "1000" => "마우스 클릭 보고",
        "1002" => "마우스 드래그 보고",
        "1003" => "마우스 모든움직임 보고",
        "1006" => "SGR 확장 좌표",
        "1015" => "urxvt 확장 좌표",
        "1004" => "포커스 보고",
        "2004" => "bracketed paste",
        "25" => "커서 표시",
        _ => "",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("사용법: modeprobe <명령> [인자...]");
        std::process::exit(2);
    }
    let size = nabi_types::GridSize::new(120, 30);
    let mut cmd = portable_pty::CommandBuilder::new(&args[0]);
    for a in &args[1..] {
        cmd.arg(a);
    }
    let pty = portable_pty::native_pty_system();
    let pair = pty
        .openpty(portable_pty::PtySize {
            rows: size.rows(),
            cols: size.cols(),
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut child = pair.slave.spawn_command(cmd).expect("spawn");
    let mut reader = pair.master.try_clone_reader().expect("reader");
    drop(pair.slave);

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
        if buf.len() > 512 * 1024 {
            break;
        }
    }
    let _ = child.kill();
    let text = String::from_utf8_lossy(&buf);

    // CSI ? <숫자;숫자...> h|l 만 추린다.
    let mut seen: Vec<(String, bool)> = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == '\u{1b}' && bytes[i + 1] == '[' && bytes[i + 2] == '?' {
            let mut j = i + 3;
            let mut nums = String::new();
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == ';') {
                nums.push(bytes[j]);
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == 'h' || bytes[j] == 'l') {
                let on = bytes[j] == 'h';
                for n in nums.split(';').filter(|s| !s.is_empty()) {
                    seen.retain(|(k, _)| k != n);
                    seen.push((n.to_string(), on));
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    println!("=== {} 이 켠 터미널 모드(최종 상태) ===", args.join(" "));
    println!("받은 바이트: {}", buf.len());
    if seen.is_empty() {
        println!("  (DEC private mode 설정 없음)");
    }
    for (n, on) in &seen {
        let l = label(n);
        let mark = if *on { "켬" } else { "끔" };
        println!("  ?{n:<5} {mark}   {l}");
    }
}
