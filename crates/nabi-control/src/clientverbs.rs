//! `nabi cli` verb 파싱 — client.rs에서 분리(라인 한도). I/O 없음(인자 → 요청).

use crate::protocol::ControlRequest;

/// verb/플래그 → 요청. 실패 시 사용법 문자열.
pub(crate) fn parse_verb(args: &[String]) -> Result<ControlRequest, String> {
    let usage = "사용법:\n  nabi cli list [--json]\n  nabi cli capture --pane <id> [--lines <n>]\n  \
        nabi cli spawn [--shell …|--ssh <세션>] [--cwd <path>] [--dock tab|split-right|split-down|new-window]\n  \
        nabi cli send --pane <id> --data <text> [--raw]\n  nabi cli kill --pane <id>\n  \
        nabi cli resize --pane <id> --cols <c> --rows <r>\n  \
        nabi cli focus --pane <id> | set-title --pane <id> --title <t> | notify --title <t> [--body <b>]\n  \
        공통: --json(머신 출력), --pane 대신 --match \"title:x,cwd:y,kind:z,state:idle\"";
    let pane = |a: &[String]| flag(a, "--pane").and_then(|s| s.parse::<u64>().ok());
    match args.first().map(String::as_str) {
        Some("agent") => crate::clientagent::parse_agent(args, usage, &pane),
        // 이벤트 구독 스트림(B3): --pane 필터 + --kind 콤마 목록.
        Some("events") => Ok(ControlRequest::Subscribe {
            pane: pane(args),
            kinds: flag(args, "--kind").map(|k| k.split(',').map(str::to_string).collect()).unwrap_or_default(),
        }),
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
        // schedule create "<spec>" --send <텍스트>|--command <명령>|--notify <텍스트> [--name N] [--pane-title T]
        Some("schedule") if args.get(1).map(String::as_str) == Some("create") => {
            let spec = args.get(2).cloned().ok_or(usage)?;
            let (kind, payload) = if let Some(v) = flag(args, "--send") { ("send", v) }
                else if let Some(v) = flag(args, "--command") { ("command", v) }
                else if let Some(v) = flag(args, "--notify") { ("notify", v) }
                else { return Err(usage.to_string()) };
            Ok(ControlRequest::ScheduleCreate {
                name: flag(args, "--name").unwrap_or_default(),
                spec,
                kind: kind.into(),
                payload,
                pane_title: flag(args, "--pane-title").unwrap_or_default(),
            })
        }
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

pub(crate) fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
