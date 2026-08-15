//! `nabi mcp` — stdio MCP 서버(제어 파이프 프록시, CP-8).
//!
//! 도구 세트는 ControlRequest와 1:1(신규 로직 없음). 호출은 동일 named pipe를
//! 경유하므로 권한 정책(off/ask/on·그룹 승인)이 자동 상속된다 — MCP라고 우회 없음.
//! 등록(Claude Code): `claude mcp add nabiterm -- nabi.exe mcp`
//! 전송: 줄 단위 JSON-RPC 2.0(MCP stdio).

use crate::protocol::{ControlRequest, ControlResponse};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// stdio 루프. pane 환경변수(NABI_CONTROL_*)가 없으면 2를 돌려준다.
pub fn run() -> i32 {
    let (Ok(pipe), Ok(token)) =
        (std::env::var("NABI_CONTROL_PIPE"), std::env::var("NABI_CONTROL_TOKEN"))
    else {
        eprintln!("nabiTerm pane 안에서 실행하세요(NABI_CONTROL_* 환경변수 없음)");
        return 2;
    };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let id = msg.get("id").cloned().filter(|v| !v.is_null());
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "nabiterm", "version": env!("CARGO_PKG_VERSION") }
            }),
            "tools/list" => json!({ "tools": tool_defs() }),
            "tools/call" => call_tool(&pipe, &token, msg.get("params")),
            "ping" => json!({}),
            _ => {
                // 알림(notifications/*)은 무응답, 미지의 요청(id 있음)은 메서드 오류.
                if let Some(id) = id {
                    let e = json!({ "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32601, "message": format!("method not found: {method}") } });
                    let _ = writeln!(out, "{e}");
                    let _ = out.flush();
                }
                continue;
            }
        };
        if let Some(id) = id {
            let envl = json!({ "jsonrpc": "2.0", "id": id, "result": result });
            let _ = writeln!(out, "{envl}");
            let _ = out.flush();
        }
    }
    0
}

/// tools/call: 도구명+인자 → ControlRequest → 파이프 왕복 → MCP content.
fn call_tool(pipe: &str, token: &str, params: Option<&Value>) -> Value {
    let p = params.cloned().unwrap_or_else(|| json!({}));
    let name = p["name"].as_str().unwrap_or("");
    let args = p.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let req = match build_request(name, &args) {
        Ok(r) => r,
        Err(e) => return tool_err(&e),
    };
    match crate::client::request(pipe, token, &req) {
        Ok(resp) => {
            let is_err = matches!(resp, ControlResponse::Err { .. });
            json!({
                "content": [{ "type": "text", "text": serde_json::to_string(&resp).unwrap_or_default() }],
                "isError": is_err
            })
        }
        Err(e) => tool_err(&e),
    }
}

fn tool_err(msg: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": msg }], "isError": true })
}

/// 도구명 + 인자(JSON) → ControlRequest. 미지 도구는 오류.
fn build_request(name: &str, a: &Value) -> Result<ControlRequest, String> {
    let s = |k: &str| a.get(k).and_then(|v| v.as_str()).map(String::from);
    let n = |k: &str| a.get(k).and_then(|v| v.as_u64());
    let need_pane = || n("pane").ok_or_else(|| "pane 인자 필요".to_string());
    Ok(match name {
        "nabi_list_panes" => ControlRequest::ListPanes,
        "nabi_capture" => ControlRequest::Capture {
            pane: need_pane()?,
            lines: n("lines").unwrap_or(50) as u32,
            start: a.get("start").and_then(|v| v.as_i64()),
            end: a.get("end").and_then(|v| v.as_i64()),
            escapes: a.get("escapes").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "nabi_spawn" => ControlRequest::SpawnTerminal {
            shell: s("shell").unwrap_or_else(|| "powershell".into()),
            cwd: s("cwd"),
            dock: s("dock"),
            ssh: s("ssh"),
        },
        "nabi_send" => ControlRequest::SendInput {
            pane: need_pane()?,
            data: s("data").ok_or("data 인자 필요")?,
            raw: a.get("raw").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "nabi_kill" => ControlRequest::ClosePane { pane: need_pane()? },
        "nabi_resize" => ControlRequest::Resize {
            pane: need_pane()?,
            cols: n("cols").ok_or("cols 인자 필요")? as u16,
            rows: n("rows").ok_or("rows 인자 필요")? as u16,
        },
        "nabi_open_browser" => ControlRequest::OpenBrowser { path: s("path") },
        "nabi_open_sftp" => {
            ControlRequest::OpenSftp { session: s("session").ok_or("session 인자 필요")? }
        }
        "nabi_agent_explain" => ControlRequest::AgentExplain { pane: need_pane()? },
        "nabi_wait" => ControlRequest::Wait {
            pane: need_pane()?,
            until: s("until").unwrap_or_else(|| "exit".into()),
            timeout_ms: n("timeout_ms").unwrap_or(60_000),
            match_text: s("match"),
            match_regex: s("regex"),
        },
        "nabi_focus" => ControlRequest::Focus { pane: need_pane()? },
        "nabi_set_title" => ControlRequest::SetTitle {
            pane: need_pane()?,
            title: s("title").ok_or("title 인자 필요")?,
        },
        "nabi_notify" => ControlRequest::Notify {
            title: s("title").ok_or("title 인자 필요")?,
            body: s("body").unwrap_or_default(),
        },
        other => return Err(format!("미지의 도구: {other}")),
    })
}

/// MCP 도구 선언(이름·설명·입력 스키마).
fn tool_defs() -> Vec<Value> {
    let t = |name: &str, desc: &str, props: Value, req: &[&str]| {
        json!({ "name": name, "description": desc,
            "inputSchema": { "type": "object", "properties": props, "required": req } })
    };
    let pane = json!({ "type": "integer", "description": "pane ID (nabi_list_panes 참조)" });
    vec![
        t("nabi_list_panes", "nabiTerm pane 목록(제목·크기·종류·cwd·활동상태·exit code)", json!({}), &[]),
        t(
            "nabi_capture",
            "pane 화면+스크롤백 텍스트 캡처. start/end=절대 줄(음수=끝에서), escapes=SGR 보존",
            json!({ "pane": pane, "lines": { "type": "integer" }, "start": { "type": "integer" },
                "end": { "type": "integer" }, "escapes": { "type": "boolean" } }),
            &["pane"],
        ),
        t(
            "nabi_spawn",
            "새 터미널 스폰(응답에 pane ID). dock=tab|split-right|split-down|new-window, ssh=저장 세션 이름",
            json!({ "shell": { "type": "string", "enum": ["powershell", "pwsh", "cmd", "wsl", "gitbash"] },
                "cwd": { "type": "string" }, "dock": { "type": "string" }, "ssh": { "type": "string" } }),
            &[],
        ),
        t(
            "nabi_send",
            "pane에 텍스트 입력. 기본은 paste-safe(bracketed paste 래핑); 실행하려면 raw=true로 개행 포함 전송",
            json!({ "pane": pane, "data": { "type": "string" }, "raw": { "type": "boolean" } }),
            &["pane", "data"],
        ),
        t("nabi_kill", "pane 종료", json!({ "pane": pane }), &["pane"]),
        t(
            "nabi_resize",
            "pane 그리드 크기 변경",
            json!({ "pane": pane, "cols": { "type": "integer" }, "rows": { "type": "integer" } }),
            &["pane", "cols", "rows"],
        ),
        t("nabi_open_browser", "로컬 파일 브라우저 탭 열기", json!({ "path": { "type": "string" } }), &[]),
        t(
            "nabi_open_sftp",
            "저장 세션 이름으로 SFTP 탭 열기(자격증명은 볼트)",
            json!({ "session": { "type": "string" } }),
            &["session"],
        ),
        t(
            "nabi_wait",
            "pane 조건 대기(exit|command-done|output|idle|agent:idle|agent:working|agent:blocked|agent:done). output은 match/regex로 화면 패턴 대기 가능",
            json!({ "pane": pane, "until": { "type": "string" }, "timeout_ms": { "type": "integer" }, "match": { "type": "string" }, "regex": { "type": "string" } }),
            &["pane"],
        ),
        t(
            "nabi_agent_explain",
            "pane 화면을 에이전트 상태 감지 규칙으로 평가한 근거 회신(디버깅)",
            json!({ "pane": pane }),
            &["pane"],
        ),
        t("nabi_focus", "pane 탭 활성화(사용자 주의 유도)", json!({ "pane": pane }), &["pane"]),
        t(
            "nabi_set_title",
            "pane 탭 제목 변경",
            json!({ "pane": pane, "title": { "type": "string" } }),
            &["pane", "title"],
        ),
        t(
            "nabi_notify",
            "사용자 토스트 알림(발신 pane 표기)",
            json!({ "title": { "type": "string" }, "body": { "type": "string" } }),
            &["title"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_requests_from_tool_calls() {
        let r = build_request("nabi_send", &json!({ "pane": 3, "data": "ls", "raw": true })).unwrap();
        assert!(matches!(r, ControlRequest::SendInput { pane: 3, raw: true, .. }));
        let r = build_request("nabi_capture", &json!({ "pane": 1, "start": -100 })).unwrap();
        assert!(matches!(r, ControlRequest::Capture { pane: 1, start: Some(-100), .. }));
        assert!(build_request("nabi_send", &json!({ "pane": 3 })).is_err());
        assert!(build_request("unknown", &json!({})).is_err());
        // 도구 선언이 전부 유효한 JSON으로 직렬화된다(+A4 agent explain).
        assert_eq!(tool_defs().len(), 13);
        let r = build_request("nabi_agent_explain", &json!({ "pane": 2 })).unwrap();
        assert!(matches!(r, ControlRequest::AgentExplain { pane: 2 }));
    }
}
