//! CLI 클라이언트 — pane 안에서 `nabi cli <verb>`로 파이프에 1요청-1응답.

use crate::protocol::{ControlRequest, ControlResponse};
use std::io::{BufRead, BufReader, Write};

/// 파이프에 접속해 인증 후 요청 하나를 보내고 응답을 받는다.
/// 서버가 다음 인스턴스를 준비하는 찰나(ERROR_PIPE_BUSY)는 잠시 재시도한다.
pub fn request(pipe: &str, token: &str, req: &ControlRequest) -> Result<ControlResponse, String> {
    let mut f = None;
    for i in 0..50 {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe) {
            Ok(h) => {
                f = Some(h);
                break;
            }
            // 231=파이프 혼잡, 2=파일 없음, 3=경로 없음(서버가 파이프를 만들기 직전) — 모두 준비 대기.
            Err(e) if i < 49 && matches!(e.raw_os_error(), Some(231) | Some(2) | Some(3)) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => return Err(format!("파이프 접속 실패({pipe}): {e}")),
        }
    }
    let f = f.ok_or_else(|| format!("파이프 접속 실패({pipe}): 시간 초과"))?;
    let mut r = BufReader::new(f.try_clone().map_err(|e| e.to_string())?);
    let mut w = f;
    let from = std::env::var("NABI_PANE_ID").ok().and_then(|s| s.parse().ok());
    let hello = ControlRequest::Hello { token: token.to_string(), from };
    for m in [&hello, req] {
        let mut s = serde_json::to_string(m).map_err(|e| e.to_string())?;
        s.push('\n');
        w.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    }
    let mut line = String::new();
    r.read_line(&mut line).map_err(|e| e.to_string())?; // Hello 응답.
    if let Ok(ControlResponse::Err { message }) = serde_json::from_str(&line) {
        return Err(message);
    }
    line.clear();
    r.read_line(&mut line).map_err(|e| e.to_string())?;
    serde_json::from_str(&line).map_err(|e| format!("응답 파싱 실패: {e}"))
}

/// `nabi cli <verb> [...]` 진입점. 종료 코드를 돌려준다(0=성공).
pub fn run_cli(args: &[String]) -> i32 {
    let (Ok(pipe), Ok(token)) = (std::env::var("NABI_CONTROL_PIPE"), std::env::var("NABI_CONTROL_TOKEN"))
    else {
        eprintln!("nabiTerm pane 안에서 실행하세요(NABI_CONTROL_* 환경변수 없음)");
        return 2;
    };
    // 머신 가독 출력(--json) + 속성 주소지정(--match → --pane 해석, CP-6).
    let json = args.iter().any(|a| a == "--json");
    let args: Vec<String> = args.iter().filter(|a| *a != "--json").cloned().collect();
    // `--match` 는 한 벌, `--match … --all` 은 맞는 pane 마다 한 벌로 펼쳐진다.
    let mut sets = match crate::matcher::expand_args(&args, || {
        match request(&pipe, &token, &ControlRequest::ListPanes) {
            Ok(ControlResponse::Panes { panes }) => Ok(panes),
            Ok(other) => Err(format!("list 응답이 아님: {other:?}")),
            Err(e) => Err(e),
        }
    }) {
        Ok(a) => a,
        Err(e) => { eprintln!("오류: {e}"); return 1; }
    };
    // 여럿으로 펼쳐졌으면 **같은 길을 pane 마다 한 번씩** 다시 탄다. 펼친 인자에는
    // `--match`·`--all` 이 없으니 다시 펼쳐지지 않는다 — 출력·오류 처리를 복제하지 않으려고
    // 이렇게 한다. 두 벌로 적으면 한쪽만 고쳐진다.
    //
    // **하나라도 실패하면 실패로 끝낸다.** 절반만 되고 성공이라고 하면 부른 쪽이 다 됐다고 믿는다.
    if sets.len() > 1 {
        let mut bad = 0;
        for mut one in sets {
            if json {
                one.push("--json".into());
            }
            if run_cli(&one) != 0 {
                bad += 1;
            }
        }
        if bad > 0 {
            eprintln!("{bad}개 pane 에서 실패");
        }
        return i32::from(bad > 0);
    }
    let args = sets.pop().unwrap_or_default();
    // B4: `layout apply --file <json>` — panes 목록을 spawn 요청으로 합성(선언적 부트스트랩).
    if args.first().map(String::as_str) == Some("layout")
        && args.get(1).map(String::as_str) == Some("apply")
    {
        return crate::clientagent::layout_apply(&pipe, &token, &args);
    }
    // C6: `security audit [--json]` — 디스크의 설정을 읽어 위험 조합 보고(보고 전용).
    if args.first().map(String::as_str) == Some("security")
        && args.get(1).map(String::as_str) == Some("audit")
    {
        let cfg = nabi_config::load(&nabi_config::StorageLayout::resolve());
        let findings = nabi_config::audit::audit(&cfg);
        if json {
            let items: Vec<_> = findings.iter().map(|f| serde_json::json!({
                "id": f.id, "severity": format!("{:?}", f.severity), "message": f.message, "fix_at": f.fix_at,
            })).collect();
            println!("{}", serde_json::json!({ "findings": items }));
        } else if findings.is_empty() {
            println!("특이사항 없음 — 기본 권한 상태입니다.");
        } else {
            for f in &findings {
                let tag = match f.severity { nabi_config::audit::Severity::Warn => "경고", _ => "정보" };
                println!("[{tag}] {}: {}  (수정: {})", f.id, f.message, f.fix_at);
            }
        }
        return 0;
    }
    // A5: `integration install|status claude` — 로컬 파일 작업(서버 왕복 없음).
    if args.first().map(String::as_str) == Some("integration") {
        let target = args.get(2).map(String::as_str).unwrap_or("claude");
        if target != "claude" {
            eprintln!("지원 대상: claude");
            return 2;
        }
        return match args.get(1).map(String::as_str) {
            Some("install") => match crate::integration::install_claude() {
                Ok(m) => { println!("{m}"); 0 }
                Err(e) => { eprintln!("오류: {e}"); 1 }
            },
            Some("status") => { println!("{}", crate::integration::status_claude()); 0 }
            _ => { eprintln!("사용법: nabi cli integration install|status claude"); 2 }
        };
    }
    // B3: `api schema` — 프로토콜 자기 문서(서버 왕복 불필요, 로컬 출력).
    if args.first().map(String::as_str) == Some("api")
        && args.get(1).map(String::as_str) == Some("schema")
    {
        println!("{}", serde_json::to_string_pretty(&crate::apidoc::api_doc()).unwrap_or_default());
        return 0;
    }
    // B1: `agent prompt` = 텍스트 전송(+Enter) 후 선택적으로 상태 도달까지 대기 — 요청 합성.
    if args.first().map(String::as_str) == Some("agent")
        && args.get(1).map(String::as_str) == Some("prompt")
    {
        return crate::clientagent::agent_prompt(&pipe, &token, &args, json);
    }
    let req = match crate::clientverbs::parse_verb(&args) {
        Ok(r) => r,
        Err(usage) => {
            eprintln!("{usage}");
            return 2;
        }
    };
    // tail/events는 무한 스트림 — 전용 경로(Ctrl+C로 종료).
    if matches!(req, ControlRequest::Tail { .. } | ControlRequest::Subscribe { .. }) {
        return match stream(&pipe, &token, &req, json) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("오류: {e}");
                1
            }
        };
    }
    let got = request(&pipe, &token, &req);
    if json {
        return match got {
            Ok(resp) => {
                println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                i32::from(matches!(resp, ControlResponse::Err { .. }))
            }
            Err(e) => {
                println!("{}", serde_json::json!({ "res": "err", "message": e }));
                1
            }
        };
    }
    match got {
        Ok(ControlResponse::Panes { panes }) => {
            println!("{:>4}  {:<5}  {:<8}  {:>4}x{:<4}  TITLE", "ID", "KIND", "STATE", "cols", "rows");
            for p in panes {
                let cwd = p.cwd.map(|c| format!("  [{c}]")).unwrap_or_default();
                println!(
                    "{:>4}  {:<5}  {:<8}  {:>4}x{:<4}  {}{cwd}",
                    p.id, p.kind, p.state, p.cols, p.rows, p.title
                );
            }
            0
        }
        // 옮긴 뒤의 자리를 알려 준다 — 위로 계속 굴리는 쪽은 이 값으로 멈출 때를 안다.
        Ok(ControlResponse::Scrolled { offset, history }) => {
            println!("offset {offset} / history {history}");
            0
        }
        // 진단 출력은 사람이 읽는 것이 목적이다 — 한 줄에 하나씩, 이름 그대로.
        Ok(ControlResponse::Modes {
            pane, alt_screen, mouse_on, alt_scroll, bracketed_paste, app_cursor, kitty_keys,
            scrollback_lines, scroll_offset, scrollback_wipes,
        }) => {
            println!("pane            {pane}");
            println!("alt_screen      {alt_screen}   (대체 화면 — 스크롤백 없음)");
            println!("mouse_on        {mouse_on}   (앱이 마우스를 직접 받는다)");
            println!("alt_scroll      {alt_scroll}   (DEC 1007 — 대체 화면에서만 유효)");
            println!("bracketed_paste {bracketed_paste}");
            println!("app_cursor      {app_cursor}");
            println!("kitty_keys      {kitty_keys}");
            println!("scrollback      {scrollback_lines}줄 (지금 {scroll_offset}줄 거슬러 봄)");
            // 0 이면 지우기 탓이 아니다 — 그 프로그램이 애초에 흘려보내지 않은 것이다.
            println!("wipes           {scrollback_wipes}회 (앱이 스크롤백을 지우려 한 횟수)");
            0
        }
        Ok(ControlResponse::Captured { text, .. }) => {
            println!("{text}");
            0
        }
        Ok(ControlResponse::Spawned { pane }) => {
            println!("{pane}"); // 새 pane ID만 출력(스크립트가 변수로 받기 쉽게).
            0
        }
        Ok(ControlResponse::Event { kind, data, .. }) => {
            if data.is_empty() {
                println!("{kind}"); // wait 조건 충족 신호(exit/idle/…).
            } else {
                println!("{data}");
            }
            0
        }
        Ok(ControlResponse::Ok) => 0,
        Ok(ControlResponse::Err { message }) => {
            eprintln!("오류: {message}");
            1
        }
        Err(e) => {
            eprintln!("오류: {e}");
            1
        }
    }
}

/// tail 등 연속 응답을 받는다(EOF까지 줄마다 출력). json=원시 NDJSON 그대로.
fn stream(pipe: &str, token: &str, req: &ControlRequest, json: bool) -> Result<(), String> {
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe)
        .map_err(|e| format!("파이프 접속 실패({pipe}): {e}"))?;
    let mut r = BufReader::new(f.try_clone().map_err(|e| e.to_string())?);
    let mut w = f;
    let from = std::env::var("NABI_PANE_ID").ok().and_then(|s| s.parse().ok());
    for m in [&ControlRequest::Hello { token: token.to_string(), from }, req] {
        let mut s = serde_json::to_string(m).map_err(|e| e.to_string())?;
        s.push('\n');
        w.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    }
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Ok(()); // 서버 종료.
        }
        if json {
            print!("{line}");
            continue;
        }
        if let Ok(ControlResponse::Event { data, kind, .. }) = serde_json::from_str(&line) {
            println!("{}", if data.is_empty() { kind } else { data });
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::clientverbs::parse_verb;
    use crate::protocol::ControlRequest;

    #[test]
    fn parses_verbs() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(matches!(parse_verb(&a(&["list"])), Ok(ControlRequest::ListPanes)));
        assert!(matches!(
            parse_verb(&a(&["capture", "--pane", "7", "--lines", "10"])),
            Ok(ControlRequest::Capture { pane: 7, lines: 10, .. })
        ));
        assert!(matches!(
            parse_verb(&a(&["capture", "--pane", "7"])),
            Ok(ControlRequest::Capture { pane: 7, lines: 50, .. })
        ));
        assert!(parse_verb(&a(&["nope"])).is_err());
        assert!(parse_verb(&a(&["capture"])).is_err());
    }
}
