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

/// 한 파일에 적용할 텍스트 편집 묶음(T6-4 rename). 열은 LSP 규약대로 UTF-16 단위.
pub struct FileEdits {
    pub path: PathBuf,
    /// (시작줄, 시작열16, 끝줄, 끝열16, 새 텍스트) — 0기반.
    pub edits: Vec<(u32, u32, u32, u32, String)>,
}

/// WorkspaceEdit → 파일별 편집 목록(changes | documentChanges 양쪽 수용).
pub(crate) fn parse_workspace_edit(v: &Value) -> Vec<FileEdits> {
    let mut out = Vec::new();
    let mut push = |uri: &str, arr: &Value| {
        let Some(path) = uri_to_path(uri) else { return };
        let edits = arr
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|e| {
                Some((
                    e["range"]["start"]["line"].as_u64()? as u32,
                    e["range"]["start"]["character"].as_u64()? as u32,
                    e["range"]["end"]["line"].as_u64()? as u32,
                    e["range"]["end"]["character"].as_u64()? as u32,
                    e["newText"].as_str()?.to_string(),
                ))
            })
            .collect::<Vec<_>>();
        if !edits.is_empty() {
            out.push(FileEdits { path, edits });
        }
    };
    if let Some(m) = v.get("changes").and_then(Value::as_object) {
        for (uri, arr) in m {
            push(uri, arr);
        }
    }
    for dc in v.get("documentChanges").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default() {
        // 파일 생성/이름변경 등 비-편집 항목은 v1에서 무시(텍스트 편집만).
        if let Some(uri) = dc["textDocument"]["uri"].as_str() {
            push(uri, &dc["edits"]);
        }
    }
    out
}

/// UTF-16 (줄, 열) → 바이트 오프셋. 줄/열이 범위를 넘으면 그 줄 끝/문서 끝으로 고정.
pub fn pos16_to_byte(text: &str, line: u32, col16: u32) -> usize {
    let mut off = 0usize;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i as u32 == line {
            let mut u16s = 0u32;
            for (bi, ch) in l.char_indices() {
                if u16s >= col16 || ch == '\n' {
                    return off + bi;
                }
                u16s += ch.len_utf16() as u32;
            }
            return off + l.trim_end_matches('\n').len();
        }
        off += l.len();
    }
    text.len()
}

/// 편집 묶음을 텍스트에 적용한다(뒤에서부터 — 앞 오프셋이 밀리지 않게). 순수.
pub fn apply_edits(text: &str, edits: &[(u32, u32, u32, u32, String)]) -> String {
    let mut sorted: Vec<_> = edits.to_vec();
    sorted.sort_by_key(|e| (e.0, e.1));
    let mut out = text.to_string();
    for (sl, sc, el, ec, new) in sorted.iter().rev() {
        let a = pos16_to_byte(&out, *sl, *sc);
        let b = pos16_to_byte(&out, *el, *ec).max(a);
        out.replace_range(a..b, new);
    }
    out
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
    fn applies_edits_utf16_safely() {
        // '한'=UTF-16 1유닛이지만 3바이트 — 바이트/UTF-16 변환이 어긋나면 여기서 깨진다.
        let text = "let 한글 = old();\nold();\n";
        // 'old'의 시작 UTF-16 열 = l,e,t,공백,한,글,공백,=,공백 다음 → 9.
        let edits = vec![(0, 9, 0, 12, "new".to_string()), (1, 0, 1, 3, "new".to_string())];
        assert_eq!(apply_edits(text, &edits), "let 한글 = new();\nnew();\n");
        // 범위 밖 열은 줄 끝으로 고정(패닉 금지).
        assert_eq!(pos16_to_byte("ab\ncd", 0, 99), 2);
        assert_eq!(pos16_to_byte("ab", 9, 0), 2, "줄 초과=문서 끝");
    }

    #[test]
    fn parses_workspace_edit_both_shapes() {
        let ch = json!({"changes": {"file:///C:/p/a.rs": [
            {"range":{"start":{"line":1,"character":2},"end":{"line":1,"character":5}},"newText":"x"}]}});
        let got = parse_workspace_edit(&ch);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].edits[0], (1, 2, 1, 5, "x".into()));
        let dc = json!({"documentChanges": [{"textDocument":{"uri":"file:///C:/p/b.rs","version":3},
            "edits":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"newText":"y"}]}]});
        assert_eq!(parse_workspace_edit(&dc).len(), 1, "documentChanges 수용");
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
