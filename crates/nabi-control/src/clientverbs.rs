//! `nabi cli` verb 파싱 — client.rs에서 분리(라인 한도). I/O 없음(인자 → 요청).

use crate::protocol::ControlRequest;

/// verb/플래그 → 요청. 실패 시 사용법 문자열.
pub(crate) fn parse_verb(args: &[String]) -> Result<ControlRequest, String> {
    let usage = "사용법:\n  nabi cli list [--json]\n  nabi cli capture --pane <id> [--lines <n>]\n  \
        nabi cli spawn [--shell …|--ssh <세션>] [--cwd <path>] [--dock tab|split-right|split-down|new-window]\n  \
        nabi cli send --pane <id> --data <text> [--raw]\n  nabi cli kill --pane <id>\n  \
        nabi cli resize --pane <id> --cols <c> --rows <r>\n  \
        nabi cli focus --pane <id> | set-title --pane <id> --title <t> | notify --title <t> [--body <b>]\n  \
        nabi cli sftp-list [--path <원격경로>] | sftp-get --remote <r> --local <l> | sftp-put --local <l> --remote <r>\n  \
        공통: --json(머신 출력), --pane 대신 --match \"title:x,cwd:y,kind:z,state:idle\"";
    let pane = |a: &[String]| flag(a, "--pane").and_then(|s| s.parse::<u64>().ok());
    match args.first().map(String::as_str) {
        Some("agent") => crate::clientagent::parse_agent(args, usage, &pane),
        // 레이아웃 export(B4). apply는 클라이언트 합성(run_cli).
        Some("layout") if args.get(1).map(String::as_str) == Some("export") => {
            Ok(ControlRequest::LayoutExport)
        }
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
        // --keys "ctrl+c enter": 키 이름을 시퀀스로 컴파일해 raw 전송(B5).
        Some("send") if flag(args, "--keys").is_some() => {
            let spec = flag(args, "--keys").unwrap_or_default();
            let bytes = crate::keyspec::compile(&spec)?;
            Ok(ControlRequest::SendInput {
                pane: pane(args).ok_or(usage)?,
                data: String::from_utf8_lossy(&bytes).into_owned(),
                raw: true,
            })
        }
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
        Some("pane-modes") => Ok(ControlRequest::PaneModes {
            pane: flag(args, "--pane").and_then(|v| v.parse().ok()).ok_or("pane-modes: --pane 이 필요합니다")?,
        }),
        Some("open-here") => Ok(ControlRequest::OpenHere {
            path: flag(args, "--path").ok_or("open-here: --path 가 필요합니다")?,
        }),
        Some("web") => Ok(ControlRequest::OpenWeb { url: flag(args, "--url") }),
        Some("open-file") => Ok(ControlRequest::OpenEditor {
            path: flag(args, "--path").ok_or("open-file: --path 가 필요합니다")?,
        }),
        // S6-55: 열린 SFTP 연결로 원격 목록/전송(에이전트 파일 왕복).
        Some("sftp-list") => Ok(ControlRequest::SftpList { path: flag(args, "--path").unwrap_or_else(|| ".".into()) }),
        Some("sftp-get") => Ok(ControlRequest::SftpGet {
            remote: flag(args, "--remote").ok_or(usage)?,
            local: flag(args, "--local").ok_or(usage)?,
        }),
        Some("sftp-put") => Ok(ControlRequest::SftpPut {
            local: flag(args, "--local").ok_or(usage)?,
            remote: flag(args, "--remote").ok_or(usage)?,
        }),
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
