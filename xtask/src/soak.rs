//! 장기 세션 soak 하니스(T4-6) — 실제 앱을 N분 동안 굴리며 생존·메모리를 관찰한다.
//!
//! 매 사이클(10초): 명령 전송→캡처 확인(응답성) + 프로세스 메모리 기록. 종료 시
//! ① 전 구간 생존 ② 응답 실패 0 ③ 메모리 증가율이 완만(최종 ≤ 초기×3 + 200MB)을 판정한다.
//! 실행: `cargo run -p xtask -- soak [분]` (기본 10분). 게이트가 아니라 수시/야간 점검용.

use crate::e2e::{json_u64, roundtrip};
use std::io::BufReader;
use std::process::ExitCode;
use std::time::{Duration, Instant};

pub fn run(minutes: Option<String>) -> ExitCode {
    let mins: u64 = minutes.and_then(|m| m.parse().ok()).unwrap_or(10);
    match soak(mins) {
        Ok(report) => {
            println!("{report}");
            println!("SOAK 통과 ({mins}분)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("SOAK 실패: {e}");
            ExitCode::FAILURE
        }
    }
}

fn mem_mb(pid: u32) -> Option<u64> {
    // tasklist CSV: "이미지","PID","세션","세션#","메모리 사용" — 마지막 필드 "12,345 K".
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let last = s.trim().rsplit('"').nth(1)?; // 마지막 따옴표 필드.
    let digits: String = last.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok().map(|k| k / 1024)
}

/// 생존 판정은 tasklist가 아니라 child.try_wait()가 권위다 — tasklist는 dist(fat LTO)
/// 같은 고부하 병행 시 3회 재시도로도 일시 실패해 두 번(v0.1.429/430)이나 크래시로 오탐했다.
/// 메모리 샘플 실패는 그냥 그 사이클을 건너뛴다(측정 누락≠사망).
fn alive(child: &mut std::process::Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn soak(mins: u64) -> Result<String, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let exe = root.join("target/debug/nabi.exe");
    if !exe.exists() {
        return Err("target/debug/nabi.exe 없음 — cargo build -p nabi-app 먼저".into());
    }
    let cfg_dir = std::env::temp_dir().join(format!("nabi-soak-{}", std::process::id()));
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;
    std::fs::write(cfg_dir.join("config.toml"), "[terminal]\ncontrol_mode = \"on\"\n").map_err(|e| e.to_string())?;
    let pipe_name = format!(r"\\.\pipe\nabi-soak-{}", std::process::id());
    let token = format!("soak-{}", std::process::id());
    let mut child = std::process::Command::new(&exe)
        .env("NABI_CONFIG_DIR", &cfg_dir)
        .env("NABI_CONTROL_PIPE", &pipe_name)
        .env("NABI_CONTROL_TOKEN", &token)
        .env("NABI_LOG", "info") // 실패 시 보존되는 logs/에서 원인 추적.
        .spawn()
        .map_err(|e| format!("앱 실행 실패: {e}"))?;
    let pid = child.id();

    let res = drive(&pipe_name, &token, &mut child, pid, mins);
    let _ = child.kill();
    let _ = child.wait();
    if res.is_err() {
        // 실패 진단용으로 격리 프로필(로그 포함)을 남긴다 — 성공 시에만 청소.
        eprintln!("진단 로그 보존: {}", cfg_dir.display());
    } else {
        let _ = std::fs::remove_dir_all(&cfg_dir);
    }
    res
}

fn drive(pipe_name: &str, token: &str, child: &mut std::process::Child, pid: u32, mins: u64) -> Result<String, String> {
    let t0 = Instant::now();
    let mut pipe = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe_name) {
            Ok(f) => break f,
            Err(_) if t0.elapsed() < Duration::from_secs(30) => std::thread::sleep(Duration::from_millis(300)),
            Err(e) => return Err(format!("파이프 접속 실패: {e}")),
        }
    };
    let mut rd = BufReader::new(pipe.try_clone().map_err(|e| e.to_string())?);
    let hello = format!(r#"{{"op":"hello","token":"{token}","from":null}}"#);
    roundtrip(&mut pipe, &mut rd, &hello)?;
    let r = roundtrip(&mut pipe, &mut rd, r#"{"op":"spawn-terminal","shell":"powershell","cwd":null}"#)?;
    let pane = json_u64(&r, "pane").ok_or("spawn 실패")?;
    std::thread::sleep(Duration::from_secs(3));

    let mem0 = mem_mb(pid).ok_or("초기 메모리 측정 실패")?;
    let (mut peak, mut cycles, mut fails) = (mem0, 0u64, 0u64);
    let deadline = Instant::now() + Duration::from_secs(mins * 60);
    while Instant::now() < deadline {
        cycles += 1;
        // 응답성: 명령을 보내고 출력이 도는지(가벼운 부하 — 스크롤백도 조금씩 쌓는다).
        let send = format!(r#"{{"op":"send-input","pane":{pane},"data":"echo SOAK_{cycles}\r","raw":false}}"#);
        let ok = roundtrip(&mut pipe, &mut rd, &send).is_ok() && {
            std::thread::sleep(Duration::from_millis(700));
            let cap = format!(r#"{{"op":"capture","pane":{pane},"lines":20}}"#);
            roundtrip(&mut pipe, &mut rd, &cap).map(|r| r.contains(&format!("SOAK_{cycles}"))).unwrap_or(false)
        };
        if !ok {
            fails += 1;
        }
        if !alive(child) {
            return Err(format!("사이클 {cycles}: 프로세스 소멸(크래시, exit={:?})", child.try_wait()));
        }
        if let Some(m) = mem_mb(pid) {
            peak = peak.max(m); // 측정 실패는 이 사이클 샘플만 건너뜀(비치명).
        }
        std::thread::sleep(Duration::from_secs(9));
    }
    let mem1 = mem_mb(pid).unwrap_or(peak); // 최종 측정 실패 시 피크로 대체(생존은 위에서 확인).
    let cap = mem0 * 3 + 200;
    let report = format!(
        "soak {mins}분: 사이클 {cycles} · 응답실패 {fails} · 메모리 {mem0}→{mem1}MB(피크 {peak}, 허용 {cap})"
    );
    if fails > 0 {
        return Err(format!("{report} — 응답 실패 있음"));
    }
    if mem1 > cap {
        return Err(format!("{report} — 메모리 증가 과다(누수 의심)"));
    }
    Ok(report)
}
