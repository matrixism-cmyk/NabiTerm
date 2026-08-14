//! TUI가 켜는 터미널 모드를 실제로 확인한다(휠 스크롤 진단용).
//!
//! `cargo run -p nabi-pty --example modeprobe -- <명령> [인자...]`
//! 앱을 PTY에 띄우고 첫 출력에서 DEC private mode 설정(`CSI ? n h/l`)만 추려 보고한다.
//! 화면에 뭐가 그려지는지가 아니라 **무엇을 켰는지**가 휠 동작을 가른다.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 자식이 `cmd /c ...`이면 실제 TUI는 손자다 — 트리째 정리해야 PTY가 풀린다.
fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    let _ = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

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

/// 환경변수로 받은 문자열의 역슬래시 표기를 실제 제어문자로 바꾼다(`\r` `\n` `\e`).
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some('e') => out.push('\u{1b}'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// `text[i..]`가 `ESC [` 로 시작할 때, 숫자와 `;`만 지난 뒤 나오는 최종 바이트가 `end`인가.
/// (`CSI 1;40 r` 같은 시퀀스를 종류별로 세기 위한 최소 판별기.)
fn tail_is(text: &str, i: usize, end: char) -> bool {
    let rest = &text[i + 2..];
    let mut it = rest.chars();
    loop {
        match it.next() {
            Some(c) if c.is_ascii_digit() || c == ';' => {}
            Some(c) => return c == end,
            None => return false,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("사용법: modeprobe <명령> [인자...]");
        std::process::exit(2);
    }
    // 크기는 조절할 수 있어야 한다 — 화면보다 긴 출력이라야 스크롤 동작을 볼 수 있다.
    let rows: u16 = std::env::var("NABI_PROBE_ROWS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let size = nabi_types::GridSize::new(120, rows);
    let mut cmd = portable_pty::CommandBuilder::new(&args[0]);
    for a in &args[1..] {
        cmd.arg(a);
    }
    // 현재 디렉터리를 물려준다 — `codex resume`처럼 프로젝트별로 상태가 다른 앱이 있다.
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd);
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

    // 시작 화면에서 입력을 기다리는 앱(신뢰 확인 등)은 답을 줘야 본 TUI로 넘어간다.
    // `NABI_PROBE_KEYS`에 보낼 문자열을 넣는다(`\r`=엔터, `\n`=줄바꿈).
    if let Ok(keys) = std::env::var("NABI_PROBE_KEYS") {
        if let Ok(mut w) = pair.master.take_writer() {
            std::thread::spawn(move || {
                // `|`로 나눈 단계를 4초 간격으로 보낸다(신뢰 확인 → 명령 입력처럼 두 번 필요할 때).
                for part in keys.split('|') {
                    std::thread::sleep(Duration::from_secs(4));
                    let _ = std::io::Write::write_all(&mut w, unescape(part).as_bytes());
                    let _ = std::io::Write::flush(&mut w);
                }
            });
        }
    }

    // 읽기는 별도 스레드에서 한다 — `read`는 데이터가 없으면 그대로 막혀서, 마감 시각만으로는
    // 루프를 빠져나올 수 없다(TUI는 첫 화면을 그린 뒤 입력만 기다리며 조용해진다).
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = buf.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            let mut b = sink.lock().unwrap();
            b.extend_from_slice(&chunk[..n]);
            if b.len() > 512 * 1024 {
                break;
            }
        }
    });
    // 기다리는 시간 — TUI는 뜨는 데 몇 초 걸리고, 모드는 첫 화면을 다 그린 뒤에 켜기도 한다.
    let secs: u64 =
        std::env::var("NABI_PROBE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    // `NABI_PROBE_RESIZE`가 있으면 마감 6초 전에 창 크기를 바꾼다.
    // 리사이즈는 앱이 화면을 다시 구성하는 계기라, 이때만 나오는 시퀀스가 있다
    // (codex의 resize_reflow는 이때 과거 기록을 스크롤백으로 되돌린다고 말한다).
    let resize: Option<u16> = std::env::var("NABI_PROBE_RESIZE").ok().and_then(|v| v.parse().ok());
    match resize.filter(|_| secs > 6) {
        Some(r) => {
            std::thread::sleep(Duration::from_secs(secs - 6));
            let _ = pair.master.resize(portable_pty::PtySize {
                rows: r,
                cols: size.cols(),
                pixel_width: 0,
                pixel_height: 0,
            });
            println!("[리사이즈: {}행 → {r}행]", size.rows());
            std::thread::sleep(Duration::from_secs(6));
        }
        None => std::thread::sleep(Duration::from_secs(secs)),
    }
    kill_tree(child.process_id());
    let _ = child.kill();
    let buf = buf.lock().unwrap().clone();
    // 원본이 필요할 때가 있다(모드 말고 무엇을 보냈는지 눈으로 봐야 하는 경우).
    if let Ok(p) = std::env::var("NABI_PROBE_RAW") {
        let _ = std::fs::write(p, &buf);
    }
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
    // 같은 바이트를 실제 화면 모델에 먹여 본다 — "휠로 볼 게 남는가"의 답은 스크롤백 크기다.
    let mut model = nabi_vt::TermModel::new(size, 5000);
    model.process(&buf);
    println!("=== {} 이 켠 터미널 모드(최종 상태) ===", args.join(" "));
    println!("받은 바이트: {}  화면: {}x{}", buf.len(), size.cols(), size.rows());
    println!("스크롤백에 쌓인 줄: {}", model.history_size());
    // 스크롤백을 만드는 수단은 셋뿐이다 — 어느 것도 안 쓰면 앱이 과거를 터미널에 안 넘긴다.
    let count = |re: &str| text.matches(re).count();
    println!(
        "스크롤 영역(DECSTBM): {}  ·  SU(CSI S): {}  ·  RI(ESC M): {}",
        text.match_indices("\u{1b}[").filter(|(i, _)| tail_is(&text, *i, 'r')).count(),
        text.match_indices("\u{1b}[").filter(|(i, _)| tail_is(&text, *i, 'S')).count(),
        count("\u{1b}M"),
    );
    if seen.is_empty() {
        println!("  (DEC private mode 설정 없음)");
    }
    for (n, on) in &seen {
        let l = label(n);
        let mark = if *on { "켬" } else { "끔" };
        println!("  ?{n:<5} {mark}   {l}");
    }
}
