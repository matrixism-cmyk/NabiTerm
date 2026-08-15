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
            Err(e) if i < 49 && matches!(e.raw_os_error(), Some(231) | Some(2)) => {
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
    let args = match crate::matcher::resolve_args(&args, || {
        match request(&pipe, &token, &ControlRequest::ListPanes) {
            Ok(ControlResponse::Panes { panes }) => Ok(panes),
            Ok(other) => Err(format!("list 응답이 아님: {other:?}")),
            Err(e) => Err(e),
        }
    }) {
        Ok(a) => a,
        Err(e) => { eprintln!("오류: {e}"); return 1; }
    };
    // B1: `agent prompt` = 텍스트 전송(+Enter) 후 선택적으로 상태 도달까지 대기 — 요청 합성.
    if args.first().map(String::as_str) == Some("agent")
        && args.get(1).map(String::as_str) == Some("prompt")
    {
        return crate::clientagent::agent_prompt(&pipe, &token, &args, json);
    }
    let req = match parse_verb(&args) {
        Ok(r) => r,
        Err(usage) => {
            eprintln!("{usage}");
            return 2;
        }
    };
    // tail은 무한 스트림 — 전용 경로(Ctrl+C로 종료).
    if matches!(req, ControlRequest::Tail { .. }) {
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

/// verb/플래그 → 요청. 실패 시 사용법 문자열.
fn parse_verb(args: &[String]) -> Result<ControlRequest, String> {
    let usage = "사용법:\n  nabi cli list [--json]\n  nabi cli capture --pane <id> [--lines <n>]\n  \
        nabi cli spawn [--shell …|--ssh <세션>] [--cwd <path>] [--dock tab|split-right|split-down|new-window]\n  \
        nabi cli send --pane <id> --data <text> [--raw]\n  nabi cli kill --pane <id>\n  \
        nabi cli resize --pane <id> --cols <c> --rows <r>\n  \
        nabi cli focus --pane <id> | set-title --pane <id> --title <t> | notify --title <t> [--body <b>]\n  \
        공통: --json(머신 출력), --pane 대신 --match \"title:x,cwd:y,kind:z,state:idle\"";
    let pane = |a: &[String]| flag(a, "--pane").and_then(|s| s.parse::<u64>().ok());
    match args.first().map(String::as_str) {
        Some("agent") => crate::clientagent::parse_agent(args, usage, &pane),
        Some("list") => Ok(ControlRequest::ListPanes),
        Some("capture") => Ok(ControlRequest::Capture {
            pane: pane(args).ok_or(usage)?,
            lines: flag(args, "--lines").and_then(|s| s.parse().ok()).unwrap_or(50),
            start: flag(args, "--start").and_then(|s| s.parse().ok()),
            end: flag(args, "--end").and_then(|s| s.parse().ok()),
            escapes: args.iter().any(|a| a == "--escapes"),
        }),
        Some("spawn") => Ok(ControlRequest::SpawnTerminal {
            shell: flag(args, "--shell").unwrap_or_else(|| "powershell".into()),
            cwd: flag(args, "--cwd"),
            dock: flag(args, "--dock"),
            ssh: flag(args, "--ssh"),
        }),
        Some("send") => Ok(ControlRequest::SendInput {
            pane: pane(args).ok_or(usage)?,
            data: flag(args, "--data").ok_or(usage)?,
            raw: args.iter().any(|a| a == "--raw"),
        }),
        Some("kill") => Ok(ControlRequest::ClosePane { pane: pane(args).ok_or(usage)? }),
        Some("resize") => Ok(ControlRequest::Resize {
            pane: pane(args).ok_or(usage)?,
            cols: flag(args, "--cols").and_then(|s| s.parse().ok()).ok_or(usage)?,
            rows: flag(args, "--rows").and_then(|s| s.parse().ok()).ok_or(usage)?,
        }),
        Some("open-browser") => Ok(ControlRequest::OpenBrowser { path: flag(args, "--path") }),
        Some("open-sftp") => {
            Ok(ControlRequest::OpenSftp { session: flag(args, "--session").ok_or(usage)? })
        }
        Some("wait") => Ok(ControlRequest::Wait {
            match_text: flag(args, "--match"),
            match_regex: flag(args, "--regex"),
            pane: pane(args).ok_or(usage)?,
            until: flag(args, "--until").unwrap_or_else(|| "exit".into()),
            timeout_ms: flag(args, "--timeout").and_then(|s| s.parse().ok()).unwrap_or(60_000),
        }),
        Some("tail") => Ok(ControlRequest::Tail { pane: pane(args).ok_or(usage)? }),
        Some("focus") => Ok(ControlRequest::Focus { pane: pane(args).ok_or(usage)? }),
        Some("set-title") => Ok(ControlRequest::SetTitle {
            pane: pane(args).ok_or(usage)?,
            title: flag(args, "--title").ok_or(usage)?,
        }),
        Some("notify") => Ok(ControlRequest::Notify {
            title: flag(args, "--title").ok_or(usage)?,
            body: flag(args, "--body").unwrap_or_default(),
        }),
        // status set <key> <value> | status clear [key] — 호출 pane의 상태 표시(상태바·탭).
        Some("status") => match args.get(1).map(|s| s.as_str()) {
            Some("set") => Ok(ControlRequest::PaneStatusSet {
                key: args.get(2).cloned().ok_or(usage)?,
                value: Some(args.get(3).cloned().unwrap_or_default()),
                ttl_ms: flag(args, "--ttl").and_then(|s| s.parse().ok()),
            }),
            Some("clear") => Ok(ControlRequest::PaneStatusSet {
                key: args.get(2).cloned().unwrap_or_default(),
                value: None,
                ttl_ms: None,
            }),
            _ => Err(usage.to_string()),
        },
        _ => Err(usage.to_string()),
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

#[cfg(test)]
mod tests {
    use super::parse_verb;
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
