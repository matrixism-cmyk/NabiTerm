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
}
