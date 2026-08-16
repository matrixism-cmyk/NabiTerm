//! LSP 수신 경로(T6-4) — 리더 루프(응답/서버요청/진단 분배)와 definition 응답 파서.
//! [`crate::lspclient`]에서 분리(파일 줄 수 한도). 상태는 전부 인자로 받는 순수-측 모듈.

use crate::lspframe::{canon_uri, encode, read_frame};
use crate::lspclient::{DefLoc, Diag, Shared};
use serde_json::{json, Value};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) fn reader_loop(
    stdout: std::process::ChildStdout,
    shared: Arc<Shared>,
    stdin: Arc<Mutex<std::process::ChildStdin>>,
) {
    let mut r = BufReader::new(stdout);
    while let Some(body) = read_frame(&mut r) {
        let Ok(v) = serde_json::from_str::<Value>(&body) else { continue };
        if let Some(id) = v.get("id").cloned() {
            if v.get("method").is_none() {
                // 응답 — result만 보관(error는 None 처리와 동일하게 result 없음).
                if let (Some(id), Ok(mut m)) = (id.as_i64(), shared.replies.lock()) {
                    m.insert(id, v.get("result").cloned().unwrap_or(Value::Null));
                }
                continue;
            }
            // 서버→클라 요청 — 응답을 안 보내면 서버가 멈춘다(rust-analyzer는
            // workspace/configuration 등을 기다림). 형태에 맞는 기본값으로 즉답.
            let result = match v.get("method").and_then(Value::as_str) {
                // 항목 수만큼 null(=서버 기본 설정 사용).
                Some("workspace/configuration") => {
                    let n = v["params"]["items"].as_array().map_or(0, Vec::len);
                    Value::Array(vec![Value::Null; n])
                }
                _ => Value::Null,
            };
            if let Ok(mut w) = stdin.lock() {
                let reply = json!({"jsonrpc":"2.0","id":id,"result":result});
                let _ = w.write_all(&encode(&reply.to_string()));
                let _ = w.flush();
            }
            continue;
        }
        if v.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics") {
            let p = &v["params"];
            let uri = canon_uri(p["uri"].as_str().unwrap_or_default());
            let list: Vec<Diag> = p["diagnostics"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|d| Diag {
                            line: d["range"]["start"]["line"].as_u64().unwrap_or(0) as u32,
                            col: d["range"]["start"]["character"].as_u64().unwrap_or(0) as u32,
                            severity: d["severity"].as_u64().unwrap_or(1) as u8,
                            message: d["message"].as_str().unwrap_or_default().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Ok(mut m) = shared.diags.lock() {
                m.insert(uri, list);
            }
        }
    }
}

/// definition 응답에서 첫 위치를 뽑는다.
pub(crate) fn parse_definition(v: &Value) -> Option<DefLoc> {
    let loc = if v.is_array() { v.get(0)? } else { v };
    let (uri, range) = if let Some(u) = loc.get("uri") {
        (u, loc.get("range")?)
    } else {
        (loc.get("targetUri")?, loc.get("targetSelectionRange")?)
    };
    let path = uri_to_path(uri.as_str()?)?;
    Some(DefLoc {
        path,
        line: range["start"]["line"].as_u64()? as u32,
        col: range["start"]["character"].as_u64()? as u32,
    })
}

/// hover 응답에서 본문 텍스트를 뽑는다(MarkupContent | MarkedString | 배열 수용).
pub(crate) fn parse_hover(v: &Value) -> Option<String> {
    let c = v.get("contents")?;
    let one = |x: &Value| -> Option<String> {
        if let Some(s) = x.as_str() {
            return Some(s.to_string());
        }
        x.get("value").and_then(Value::as_str).map(str::to_string)
    };
    let text = if let Some(arr) = c.as_array() {
        arr.iter().filter_map(one).collect::<Vec<_>>().join("\n\n")
    } else {
        one(c)?
    };
    (!text.trim().is_empty()).then_some(text)
}

/// references 응답(Location[])을 위치 목록으로.
pub(crate) fn parse_locations(v: &Value) -> Vec<DefLoc> {
    v.as_array().map(Vec::as_slice).unwrap_or_default().iter().filter_map(parse_definition).collect()
}

/// file:// URI → 로컬 경로.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file:///").or_else(|| uri.strip_prefix("file://"))?;
    let dec = rest.replace("%20", " ").replace("%23", "#").replace("%3F", "?");
    Some(PathBuf::from(dec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_definition_shapes() {
        let loc = json!({"uri":"file:///C:/p/a.rs","range":{"start":{"line":3,"character":7},"end":{"line":3,"character":9}}});
        let d = parse_definition(&loc).unwrap();
        assert_eq!((d.line, d.col), (3, 7));
        assert!(d.path.to_string_lossy().ends_with("a.rs"));
        let arr = json!([loc]);
        assert!(parse_definition(&arr).is_some(), "Location[] 수용");
        let link = json!([{"targetUri":"file:///C:/p/b.rs","targetSelectionRange":{"start":{"line":1,"character":2},"end":{"line":1,"character":3}}}]);
        assert_eq!(parse_definition(&link).unwrap().line, 1, "LocationLink 수용");
    }

    #[test]
    fn parses_hover_shapes() {
        let mk = json!({"contents": {"kind": "markdown", "value": "```rust\nfn helper() -> i32\n```"}});
        assert!(parse_hover(&mk).unwrap().contains("fn helper"), "MarkupContent");
        let arr = json!({"contents": ["첫", {"language": "rust", "value": "둘"}]});
        assert_eq!(parse_hover(&arr).unwrap(), "첫\n\n둘", "MarkedString 배열");
        assert!(parse_hover(&json!({"contents": ""})).is_none(), "빈 본문은 None");
    }

    #[test]
    fn parses_reference_list() {
        let v = json!([
            {"uri":"file:///C:/p/a.rs","range":{"start":{"line":0,"character":3},"end":{"line":0,"character":9}}},
            {"uri":"file:///C:/p/b.rs","range":{"start":{"line":9,"character":0},"end":{"line":9,"character":6}}}
        ]);
        let locs = parse_locations(&v);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[1].line, 9);
        assert!(parse_locations(&Value::Null).is_empty(), "null=빈 목록");
    }
}
