//! E2E 스모크 게이트(T4-2) — 진짜 앱을 띄워 제어 평면으로 "기동→pane→입력→캡처→종료"를 검증.
//!
//! 단위테스트가 못 잡는 결합부(창 기동·오케스트레이터·PTY·제어 서버)를 실제 실행으로 본다.
//! 격리: NABI_CONFIG_DIR=임시 폴더(사용자 설정 불침), 파이프/토큰은 우리가 미리 심는다
//! (main.rs가 "이미 설정돼 있으면 존중" — 외부 하니스용으로 마련된 경로).
//!
//! 실행: `cargo run -p xtask -- e2e [exe경로]` (기본 target\debug\nabi.exe)

use std::io::{BufRead, BufReader, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// 파이프에 한 요청을 쓰고 응답 한 줄을 받는다(라인 구분 JSON).
fn roundtrip(pipe: &mut std::fs::File, rd: &mut impl BufRead, req: &str) -> Result<String, String> {
    pipe.write_all(req.as_bytes()).and_then(|_| pipe.write_all(b"\n")).map_err(|e| e.to_string())?;
    pipe.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    rd.read_line(&mut line).map_err(|e| e.to_string())?;
    if line.is_empty() {
        return Err("파이프가 응답 없이 닫혔습니다".into());
    }
    Ok(line)
}

/// JSON 응답에서 `"key":<숫자>`를 뽑는다(파서 의존성 없이 — 형식은 우리 서버가 고정).
fn json_u64(s: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\":");
    let at = s.find(&pat)? + pat.len();
    let rest: String = s[at..].chars().take_while(|c| c.is_ascii_digit()).collect();
    rest.parse().ok()
}

pub fn run(exe: Option<String>) -> ExitCode {
    match smoke(exe) {
        Ok(()) => {
            println!("E2E 스모크 통과 (기동→spawn→send→capture→종료)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("E2E 스모크 실패: {e}");
            ExitCode::FAILURE
        }
    }
}

fn smoke(exe: Option<String>) -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let exe = exe.map(std::path::PathBuf::from).unwrap_or(root.join("target/debug/nabi.exe"));
    if !exe.exists() {
        return Err(format!("실행 파일 없음: {} (cargo build -p nabi-app 먼저)", exe.display()));
    }
    // 격리 프로필: 임시 설정 폴더 + 제어 무승인(on) — 헤드리스라 확인 모달을 누를 손이 없다.
    let cfg_dir = std::env::temp_dir().join(format!("nabi-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;
    std::fs::write(cfg_dir.join("config.toml"), "[terminal]\ncontrol_mode = \"on\"\n")
        .map_err(|e| e.to_string())?;
    let pipe_name = format!(r"\\.\pipe\nabi-e2e-{}", std::process::id());
    let token = format!("e2e-{}-{:x}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));

    let mut child = std::process::Command::new(&exe)
        .env("NABI_CONFIG_DIR", &cfg_dir)
        .env("NABI_CONTROL_PIPE", &pipe_name)
        .env("NABI_CONTROL_TOKEN", &token)
        .spawn()
        .map_err(|e| format!("앱 실행 실패: {e}"))?;

    let result = drive(&pipe_name, &token);
    let _ = child.kill(); // 스모크 목적 달성 — 강제 종료로 정리(워크스페이스 저장은 검증 대상 아님).
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&cfg_dir);
    result
}

/// 파이프 접속(재시도) 후 스모크 시나리오를 몬다.
fn drive(pipe_name: &str, token: &str) -> Result<(), String> {
    // 앱 기동 + 서버 리슨까지 기다린다(첫 프레임 이후) — 최대 30초.
    let t0 = Instant::now();
    let mut pipe = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe_name) {
            Ok(f) => break f,
            Err(_) if t0.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(format!("제어 파이프 접속 실패(30초): {e}")),
        }
    };
    let mut rd = BufReader::new(pipe.try_clone().map_err(|e| e.to_string())?);
    let hello = format!(r#"{{"op":"hello","token":"{token}","from":null}}"#);
    let r = roundtrip(&mut pipe, &mut rd, &hello)?;
    if !r.contains(r#""res":"ok""#) {
        return Err(format!("hello 거부: {r}"));
    }
    let r = roundtrip(&mut pipe, &mut rd, r#"{"op":"spawn-terminal","shell":"powershell","cwd":null}"#)?;
    let pane = json_u64(&r, "pane").ok_or_else(|| format!("spawn 실패: {r}"))?;
    // 셸 프롬프트가 뜰 시간을 잠깐 주고 에코 명령을 넣는다.
    std::thread::sleep(Duration::from_secs(3));
    let send = format!(r#"{{"op":"send-input","pane":{pane},"data":"echo NABI_E2E_OK\r","raw":false}}"#);
    let r = roundtrip(&mut pipe, &mut rd, &send)?;
    if !r.contains(r#""res":"ok""#) {
        return Err(format!("send 실패: {r}"));
    }
    // 출력이 돌아올 때까지 캡처를 폴링(최대 20초). 에코된 명령줄이 아니라 실행 결과를 본다.
    let t1 = Instant::now();
    loop {
        let cap = format!(r#"{{"op":"capture","pane":{pane},"lines":50}}"#);
        let r = roundtrip(&mut pipe, &mut rd, &cap)?;
        // JSON 문자열 안의 "NABI_E2E_OK"가 명령 에코 줄 말고도 한 번 더(출력) 있으면 성공.
        if r.matches("NABI_E2E_OK").count() >= 2 {
            let close = format!(r#"{{"op":"close-pane","pane":{pane}}}"#);
            let _ = roundtrip(&mut pipe, &mut rd, &close)?;
            return Ok(());
        }
        if t1.elapsed() > Duration::from_secs(20) {
            return Err(format!("출력 대기 시간 초과 — 마지막 캡처: {r}"));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::json_u64;

    #[test]
    fn extracts_pane_number() {
        assert_eq!(json_u64(r#"{"res":"spawned","pane":42}"#, "pane"), Some(42));
        assert_eq!(json_u64(r#"{"res":"ok"}"#, "pane"), None);
    }
}
