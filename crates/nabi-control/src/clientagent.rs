//! `nabi cli agent …` — 에이전트 시맨틱 verb(A2/A4/B1). client.rs에서 분리(라인 한도).

use crate::client::request;
use crate::protocol::{ControlRequest, ControlResponse};

/// `agent report|release|explain|wait` 파싱(prompt는 run_cli가 합성 — [`agent_prompt`]).
pub(crate) fn parse_agent(
    args: &[String],
    usage: &str,
    pane: &dyn Fn(&[String]) -> Option<u64>,
) -> Result<ControlRequest, String> {
    // report/release는 훅이 자기 pane 상태를 발행하는 권위 채널 —
    // 발행이 있으면 화면 감지가 물러난다(agentwatch).
    match args.get(1).map(String::as_str) {
        Some("report") => Ok(ControlRequest::PaneStatusSet {
            key: "state".into(),
            value: Some(flag(args, "--state").ok_or(usage)?),
            ttl_ms: flag(args, "--ttl").and_then(|s| s.parse().ok()),
        }),
        Some("release") => Ok(ControlRequest::PaneStatusSet { key: "state".into(), value: None, ttl_ms: None }),
        Some("explain") => Ok(ControlRequest::AgentExplain { pane: pane(args).ok_or(usage)? }),
        Some("wait") => Ok(ControlRequest::Wait {
            pane: pane(args).ok_or(usage)?,
            until: format!("agent:{}", flag(args, "--until").unwrap_or_else(|| "idle".into())),
            timeout_ms: flag(args, "--timeout").and_then(|s| s.parse().ok()).unwrap_or(600_000),
            match_text: None,
            match_regex: None,
        }),
        // prompt는 복합 동작(전송+대기) — run_cli가 요청 두 개로 합성한다.
        _ => Err(usage.to_string()),
    }
}

/// `agent prompt --pane N --data <텍스트> [--wait [--until <state>]] [--timeout <ms>]`.
///
/// 전송과 Enter, 대기를 클라이언트에서 합성한다 — 서버에 복합 verb를 만들면 승인 그룹
/// (Inject)과 스트림 경로(Wait)가 뒤엉킨다. 요청 두 개가 더 단순하고 정책도 그대로 탄다.
pub(crate) fn agent_prompt(pipe: &str, token: &str, args: &[String], json: bool) -> i32 {
    let Some(pane) = flag(args, "--pane").and_then(|s| s.parse::<u64>().ok()) else {
        eprintln!("--pane 필요");
        return 2;
    };
    let Some(text) = flag(args, "--data") else {
        eprintln!("--data 필요");
        return 2;
    };
    // 본문은 bracketed 보호 경로, Enter는 별도 raw(붙이면 201~ 뒤 개행이 앱마다 달리 해석됨).
    for req in [
        ControlRequest::SendInput { pane, data: text, raw: false },
        ControlRequest::SendInput { pane, data: "\r".into(), raw: true },
    ] {
        match request(pipe, token, &req) {
            Ok(ControlResponse::Err { message }) | Err(message) => {
                eprintln!("오류: {message}");
                return 1;
            }
            Ok(_) => {}
        }
    }
    if !args.iter().any(|a| a == "--wait") {
        if json { println!("{}", serde_json::json!({ "res": "ok" })); } else { println!("전송됨"); }
        return 0;
    }
    let wait = ControlRequest::Wait {
        pane,
        until: format!("agent:{}", flag(args, "--until").unwrap_or_else(|| "idle".into())),
        timeout_ms: flag(args, "--timeout").and_then(|s| s.parse().ok()).unwrap_or(600_000),
        match_text: None,
        match_regex: None,
    };
    match request(pipe, token, &wait) {
        Ok(resp) => {
            if json {
                println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            } else if let ControlResponse::Event { kind, data, .. } = &resp {
                println!("{kind}: {data}");
            }
            i32::from(matches!(resp, ControlResponse::Err { .. }))
        }
        Err(e) => {
            eprintln!("오류: {e}");
            1
        }
    }
}


fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
