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
        Some("screenshot") => Ok(ControlRequest::Screenshot {
            pane: flag(args, "--pane").and_then(|v| v.parse().ok()),
            out: flag(args, "--out"),
        }),
        Some("progress") => Ok(ControlRequest::Progress {
            pane: flag(args, "--pane").and_then(|v| v.parse().ok()).ok_or("progress: --pane 이 필요합니다")?,
            // --pct 를 빼면 "이제 없음"이라는 뜻이다. 끝났을 때 지우라고 말할 길이 있어야 한다.
            percent: flag(args, "--pct").and_then(|v| v.parse().ok()),
        }),
        Some("web-list") => Ok(ControlRequest::WebList),
        Some("history") => Ok(ControlRequest::ShowHistory { pane: pane(args) }),
        // 웹 조종 낱말들 — 부르는 쪽에는 아홉 개, 프로토콜에는 하나로 모인다.
        Some(w) if web_act(w).is_some() => Ok(ControlRequest::WebAct {
            pane: pane(args),
            act: web_act(w).unwrap_or_default().to_string(),
            arg: web_arg(w, args)?,
        }),
        Some("web-eval") => Ok(ControlRequest::WebEval {
            pane: pane(args),
            js: flag(args, "--js").ok_or("--js <자바스크립트> 가 필요하다")?,
        }),
        Some("update") => Ok(ControlRequest::SelfUpdate {
            check: args.iter().any(|a| a == "--check"),
        }),
        Some("web") => Ok(ControlRequest::OpenWeb {
            url: flag(args, "--url"),
            window: args.iter().any(|a| a == "--window"),
        }),
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

/// `web-back` 같은 낱말을 동작 이름으로. 우리 것이 아니면 None.
pub(crate) fn web_act(word: &str) -> Option<&'static str> {
    // 소스가 곧 목록이다 — 여기 없는 낱말은 파지 않는다.
    const ACTS: [&str; 9] = [
        "back", "forward", "reload", "stop", "goto", "zoom", "shot", "pdf", "text",
    ];
    let rest = word.strip_prefix("web-")?;
    ACTS.into_iter().find(|a| *a == rest)
}

/// 그 낱말이 요구하는 딸린 값. 없어도 되는 것은 빈 글.
fn web_arg(word: &str, args: &[String]) -> Result<String, String> {
    match web_act(word) {
        Some("goto") => flag(args, "--url").ok_or_else(|| "--url <주소> 가 필요하다".into()),
        Some("zoom") => flag(args, "--set")
            .ok_or_else(|| "--set <배율> 이 필요하다 (1.0 = 100%)".into()),
        // 저장 자리는 안 주면 앱이 임시 폴더에 짓고 어디에 뒀는지 알려 준다.
        Some("shot") | Some("pdf") => Ok(flag(args, "--out").unwrap_or_default()),
        _ => Ok(String::new()),
    }
}

pub(crate) fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
