//! LSP 자동완성 파싱(T6-4 3단계) — CompletionList/Item, 스니펫 평문화. lspread에서 분리.

use serde_json::Value;

/// 자동완성 후보 하나(T6-4 3단계).
#[derive(Clone, Debug)]
pub struct CompItem {
    pub label: String,
    /// 삽입 텍스트(스니펫 자리표시자 `$0`/`${n:…}`는 제거된 평문).
    pub insert: String,
    /// 대치 범위(textEdit.range | replace). None이면 커서 앞 단어를 대치.
    pub range: Option<(u32, u32, u32, u32)>,
    /// 우측 보조 표기(타입/시그니처).
    pub detail: String,
}

/// LSP 스니펫 문법을 평문으로: `$0`·`$1` 제거, `${n:기본}`→기본, `${n}`→빈 문자열.
pub fn strip_snippet(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match it.peek() {
            Some('{') => {
                it.next();
                let mut body = String::new();
                let mut depth = 1;
                for d in it.by_ref() {
                    if d == '{' { depth += 1; }
                    if d == '}' { depth -= 1; if depth == 0 { break; } }
                    body.push(d);
                }
                // "n:기본값" 꼴이면 기본값만 남긴다.
                if let Some(i) = body.find(':') {
                    out.push_str(&body[i + 1..]);
                }
            }
            Some(d) if d.is_ascii_digit() => {
                while it.peek().is_some_and(char::is_ascii_digit) {
                    it.next();
                }
            }
            _ => out.push('$'),
        }
    }
    out
}

/// completion 응답(CompletionList | CompletionItem[]) → 후보 목록(sortText 순, 상한 50).
pub fn parse_completion(v: &Value) -> Vec<CompItem> {
    let items = v.get("items").and_then(Value::as_array).or_else(|| v.as_array());
    let mut list: Vec<(&Value, String)> = items
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|it| (it, it["sortText"].as_str().unwrap_or_else(|| it["label"].as_str().unwrap_or("")).to_string()))
        .collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    list.iter()
        .take(50)
        .filter_map(|(it, _)| {
            let label = it["label"].as_str()?.to_string();
            let te = it.get("textEdit");
            let raw = te
                .and_then(|t| t["newText"].as_str())
                .or_else(|| it["insertText"].as_str())
                .unwrap_or(&label)
                .to_string();
            // range | InsertReplaceEdit(replace) 둘 다 수용.
            let r = te.and_then(|t| t.get("range").or_else(|| t.get("replace")));
            let range = r.and_then(|r| {
                Some((
                    r["start"]["line"].as_u64()? as u32,
                    r["start"]["character"].as_u64()? as u32,
                    r["end"]["line"].as_u64()? as u32,
                    r["end"]["character"].as_u64()? as u32,
                ))
            });
            Some(CompItem {
                label,
                insert: strip_snippet(&raw),
                range,
                detail: it["detail"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect()
}


/// 완성 확정(순수): 앵커..커서(문자 오프셋)를 삽입 텍스트로 대치한 새 텍스트와 새 커서 위치.
pub fn commit_completion(text: &str, anchor: usize, cur: usize, insert: &str) -> (String, usize) {
    let byte_at = |ch: usize| text.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(text.len());
    let (a, b) = (byte_at(anchor.min(cur)), byte_at(cur.max(anchor)));
    let mut out = String::with_capacity(text.len() + insert.len());
    out.push_str(&text[..a]);
    out.push_str(insert);
    out.push_str(&text[b..]);
    (out, anchor.min(cur) + insert.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_snippet_placeholders() {
        assert_eq!(strip_snippet("push($0)"), "push()");
        assert_eq!(strip_snippet("map(${1:f})"), "map(f)");
        assert_eq!(strip_snippet("plain$1"), "plain");
        assert_eq!(strip_snippet("가$0격"), "가격", "한글 경계 안전");
    }

    #[test]
    fn commit_replaces_prefix() {
        let (t, c) = commit_completion("let x = he;", 8, 10, "helper()");
        assert_eq!(t, "let x = helper();");
        assert_eq!(c, 16);
        let (t2, c2) = commit_completion("가.na", 2, 4, "나비함수");
        assert_eq!(t2, "가.나비함수");
        assert_eq!(c2, 6, "한글 문자 오프셋");
    }

    #[test]
    fn parses_completion_list_and_sorts() {
        let v = json!({"items": [
            {"label": "zzz", "sortText": "b", "insertText": "zzz"},
            {"label": "helper", "sortText": "a",
             "textEdit": {"range": {"start": {"line": 1, "character": 4}, "end": {"line": 1, "character": 7}}, "newText": "helper($0)"},
             "detail": "fn() -> i32"}
        ]});
        let got = parse_completion(&v);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].label, "helper", "sortText 순");
        assert_eq!(got[0].insert, "helper()", "스니펫 제거");
        assert_eq!(got[0].range, Some((1, 4, 1, 7)));
        assert!(parse_completion(&Value::Null).is_empty());
    }
}
