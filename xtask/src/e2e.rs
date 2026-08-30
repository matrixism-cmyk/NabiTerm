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
pub(crate) fn roundtrip(pipe: &mut std::fs::File, rd: &mut impl BufRead, req: &str) -> Result<String, String> {
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
pub(crate) fn json_u64(s: &str, key: &str) -> Option<u64> {
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

pub(crate) fn smoke(exe: Option<String>) -> Result<(), String> {
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
        .env("NABI_LOG", "info") // 파일 로그를 남겨 종료 후 오류 스캔(사용자 요청 2026-08-16).
        .spawn()
        .map_err(|e| format!("앱 실행 실패: {e}"))?;

    let result = drive(&pipe_name, &token);
    let _ = child.kill(); // 스모크 목적 달성 — 강제 종료로 정리(워크스페이스 저장은 검증 대상 아님).
    let _ = child.wait();
    // 실행 중 남은 로그에서 오류/패닉을 스캔 — 스모크가 "성공"이어도 내부 오류를 잡는다.
    let scan = scan_logs(&cfg_dir);
    let _ = std::fs::remove_dir_all(&cfg_dir);
    result.and(scan)
}

/// 격리 설정 폴더의 logs/에서 ERROR·panic 줄을 찾는다(발견=실패, WARN은 보고만).
pub(crate) fn scan_logs(cfg_dir: &std::path::Path) -> Result<(), String> {
    let dir = cfg_dir.join("logs");
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(()) }; // 로그 없음=통과(구버전 호환).
    let (mut errs, mut warns) = (Vec::new(), 0usize);
    for f in rd.flatten() {
        let Ok(text) = std::fs::read_to_string(f.path()) else { continue };
        for line in text.lines() {
            if line.contains("ERROR") || line.contains("panicked") {
                errs.push(line.to_string());
            } else if line.contains(" WARN ") {
                warns += 1;
            }
        }
    }
    if warns > 0 {
        eprintln!("로그 WARN {warns}건(비차단)");
    }
    if errs.is_empty() {
        Ok(())
    } else {
        let head: Vec<_> = errs.iter().take(5).cloned().collect();
        Err(format!("로그 오류 {}건:\n{}", errs.len(), head.join("\n")))
    }
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
            open_big_file(&mut pipe, &mut rd)?; // 흘려 읽기 편집기가 실제로 뜨는지.
            // 제어 동사를 전수로 던져 본다 — 이름만 맞고 죽어 있는 것을 잡는다.
            crate::e2everbs::sweep(&mut pipe, &mut rd, pane)?;
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

/// 큰 파일을 **실제로 앱에서** 열어 흘려 읽기 편집기가 뜨고 살아 있는지 본다.
///
/// 단위 시험은 엔진만 본다. 화면은 UI 스레드에서 도는 코드라, 거기서 색인이 벗어나면
/// 렌더러가 그 자리에서 죽는다(v0.1.41에 실제로 그랬다 — painter 인덱스 초과로 앱 즉사).
/// 그래서 이 확인만은 진짜로 띄워서 해야 한다. 창을 볼 수는 없으니 살아 있는지로 판정한다.
fn open_big_file(pipe: &mut std::fs::File, rd: &mut BufReader<std::fs::File>) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("nabi-e2e-big-{}.log", std::process::id()));
    {
        use std::io::Write;
        let f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
        // 64MB(HUGE_THRESHOLD)를 넘겨야 흘려 읽기 편집기로 간다.
        for i in 0..1_800_000u32 {
            writeln!(w, "{i:08} e2e line for the streaming editor smoke test").map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())?;
    }
    // JSON 문자열이라 경로의 역슬래시를 두 겹으로 바꿔 넣는다.
    let esc = path.display().to_string().replace('\\', "\\\\");
    // 프로토콜 op 이름은 `open-editor`다(CLI 동사 `open-file`과 다르다 — 여기서 한 번 틀렸다).
    let req = format!(r#"{{"op":"open-editor","path":"{esc}"}}"#);
    let r = roundtrip(pipe, rd, &req)?;
    if !r.contains(r#""res":"ok""#) {
        let _ = std::fs::remove_file(&path);
        return Err(format!("open-editor 거부: {r}"));
    }
    // 여는 동안(줄 인덱스 스캔) 잠깐 기다린 뒤, 앱이 아직 응답하는지로 살아 있음을 본다.
    std::thread::sleep(Duration::from_secs(4));
    let alive = roundtrip(pipe, rd, r#"{"op":"list-panes"}"#)?;
    let _ = std::fs::remove_file(&path);
    if alive.contains(r#""res""#) {
        Ok(())
    } else {
        Err(format!("큰 파일을 연 뒤 앱이 응답하지 않음: {alive}"))
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
