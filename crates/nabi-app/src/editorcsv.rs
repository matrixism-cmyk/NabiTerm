//! nabiPad 데이터 형식 변환(EmEditor CSV/VS Code Rainbow CSV 벤치마킹) — CSV↔JSON/TSV/표·정렬·SQL·전치,
//! JSON↔TOML. 따옴표 인지 CSV 파서 기반. 모두 순수 함수.

use nabi_i18n::{tr, Lang};
use serde_json::Value;

/// RFC4180식 CSV 파싱(따옴표·`""` 이스케이프·따옴표 내 개행 지원).
pub(crate) fn parse_csv(s: &str, delim: char) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut field, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    field.push('"');
                    it.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else if c != '\r' {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// 필드를 CSV 규칙으로 인용(구분자/따옴표/개행 포함 시).
fn csv_field(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(crate) fn write_csv(rows: &[Vec<String>], delim: char) -> String {
    let d = delim.to_string();
    rows.iter()
        .map(|r| r.iter().map(|f| csv_field(f, delim)).collect::<Vec<_>>().join(&d))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ncols(rows: &[Vec<String>]) -> usize {
    rows.iter().map(Vec::len).max().unwrap_or(0)
}

fn cell(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// CSV(첫 행=헤더) → JSON 객체 배열.
pub(crate) fn csv_to_json(t: &str) -> String {
    let rows = parse_csv(t, ',');
    let Some(headers) = rows.first() else { return t.to_string() };
    let arr: Vec<Value> = rows[1..]
        .iter()
        .map(|r| {
            let mut m = serde_json::Map::new();
            for (i, h) in headers.iter().enumerate() {
                m.insert(h.clone(), Value::String(r.get(i).cloned().unwrap_or_default()));
            }
            Value::Object(m)
        })
        .collect();
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| t.to_string())
}

/// JSON 객체 배열 → CSV(키 합집합을 헤더로).
pub(crate) fn json_to_csv(t: &str) -> String {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(t) else { return t.to_string() };
    let mut headers: Vec<String> = Vec::new();
    for v in &arr {
        if let Value::Object(m) = v {
            for k in m.keys() {
                if !headers.contains(k) {
                    headers.push(k.clone());
                }
            }
        }
    }
    let mut rows = vec![headers.clone()];
    for v in &arr {
        if let Value::Object(m) = v {
            rows.push(headers.iter().map(|h| m.get(h).map(cell).unwrap_or_default()).collect());
        }
    }
    write_csv(&rows, ',')
}

/// CSV → JSON Lines(NDJSON, 한 줄에 객체 하나).
pub(crate) fn csv_to_ndjson(t: &str) -> String {
    let rows = parse_csv(t, ',');
    let Some(headers) = rows.first() else { return t.to_string() };
    rows[1..]
        .iter()
        .map(|r| {
            let mut m = serde_json::Map::new();
            for (i, h) in headers.iter().enumerate() {
                m.insert(h.clone(), Value::String(r.get(i).cloned().unwrap_or_default()));
            }
            serde_json::to_string(&Value::Object(m)).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}


/// CSV → 마크다운 표.
pub(crate) fn csv_to_md(t: &str) -> String {
    let rows = parse_csv(t, ',');
    if rows.is_empty() {
        return t.to_string();
    }
    let n = ncols(&rows);
    let line = |r: &[String]| {
        let mut c: Vec<String> = r.iter().map(|x| x.replace('|', "\\|").replace('\n', " ")).collect();
        c.resize(n, String::new());
        format!("| {} |", c.join(" | "))
    };
    let mut out = vec![line(&rows[0]), format!("| {} |", vec!["---"; n].join(" | "))];
    out.extend(rows[1..].iter().map(|r| line(r)));
    out.join("\n")
}

/// CSV → HTML 표(셀은 HTML 이스케이프).
pub(crate) fn csv_to_html(t: &str) -> String {
    let rows = parse_csv(t, ',');
    if rows.is_empty() {
        return t.to_string();
    }
    let esc = crate::editorconvert::html_encode;
    let mut out = String::from("<table>\n");
    for (ri, r) in rows.iter().enumerate() {
        let tag = if ri == 0 { "th" } else { "td" };
        let cells: String = r.iter().map(|f| format!("<{tag}>{}</{tag}>", esc(f))).collect();
        out.push_str(&format!("  <tr>{cells}</tr>\n"));
    }
    out.push_str("</table>");
    out
}

/// CSV 열을 같은 폭으로 정렬해 보기 좋게(읽기용).
pub(crate) fn csv_align(t: &str) -> String {
    let rows = parse_csv(t, ',');
    if rows.is_empty() {
        return t.to_string();
    }
    let n = ncols(&rows);
    let mut w = vec![0usize; n];
    for r in &rows {
        for (i, f) in r.iter().enumerate() {
            w[i] = w[i].max(f.chars().count());
        }
    }
    rows.iter()
        .map(|r| {
            (0..n)
                .map(|i| format!("{:<width$}", r.get(i).map(String::as_str).unwrap_or(""), width = w[i]))
                .collect::<Vec<_>>()
                .join(" | ")
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// CSV(헤더 포함) → SQL INSERT 문(테이블명 `t`).
pub(crate) fn csv_to_sql(t: &str) -> String {
    let rows = parse_csv(t, ',');
    if rows.len() < 2 {
        return t.to_string();
    }
    let cols = rows[0].join(", ");
    rows[1..]
        .iter()
        .map(|r| {
            let vals: Vec<String> = r.iter().map(|v| crate::editordev2::sql_escape(v)).collect();
            format!("INSERT INTO t ({cols}) VALUES ({});", vals.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// CSV 행↔열 전치.
pub(crate) fn csv_transpose(t: &str) -> String {
    let rows = parse_csv(t, ',');
    if rows.is_empty() {
        return t.to_string();
    }
    let n = ncols(&rows);
    let trans: Vec<Vec<String>> =
        (0..n).map(|c| rows.iter().map(|r| r.get(c).cloned().unwrap_or_default()).collect()).collect();
    write_csv(&trans, ',')
}

/// CSV 데이터 행을 첫 열 기준으로 정렬한다(헤더는 유지, 안정 정렬).
pub(crate) fn csv_sort_rows(t: &str) -> String {
    let mut rows = parse_csv(t, ',');
    if rows.len() <= 1 {
        return t.to_string();
    }
    rows[1..].sort_by(|a, b| a.first().map(String::as_str).unwrap_or("").cmp(b.first().map(String::as_str).unwrap_or("")));
    write_csv(&rows, ',')
}

/// "데이터" 서브메뉴 — CSV 계열/JSON·TOML 하위 그룹으로 분류(2단계 계층).
pub(crate) fn data_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editmenugroups::pick;
    use crate::editordev2 as d2;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.csvgroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.csv2json", csv_to_json), ("editor.json2csv", json_to_csv),
            ("editor.csv2ndjson", csv_to_ndjson),
            ("editor.csv2tsv", crate::editorcsv2::csv_to_tsv), ("editor.tsv2csv", crate::editorcsv2::tsv_to_csv),
            ("editor.csv2md", csv_to_md), ("editor.md2csv", crate::editorcsv2::md_to_csv), ("editor.csv2html", csv_to_html),
            ("editor.csvalign", csv_align), ("editor.csv2sql", csv_to_sql),
            ("editor.csvtranspose", csv_transpose), ("editor.csvsort", csv_sort_rows),
        ]));
    });
    ui.menu_button(tr(lang, "editor.jsongroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.json2toml", d2::json_to_toml), ("editor.toml2json", d2::toml_to_json),
            ("editor.jsonsortkeys", d2::json_sort_keys),
            ("editor.lines2json", d2::lines_to_json_array), ("editor.json2lines", d2::json_array_to_lines),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_json_roundtrip() {
        let csv = "name,note\nAlice,\"hi, there\"\nBob,x";
        let j = csv_to_json(csv);
        assert!(j.contains("\"note\": \"hi, there\""));
        assert_eq!(json_to_csv(&j), csv); // 따옴표 보존 왕복.
    }

    #[test]
    fn formats() {
        assert_eq!(crate::editorcsv2::csv_to_tsv("a,b\nc,d"), "a\tb\nc\td");
        assert_eq!(crate::editorcsv2::tsv_to_csv("a\tb"), "a,b");
        assert_eq!(csv_to_md("h1,h2\nv1,v2"), "| h1 | h2 |\n| --- | --- |\n| v1 | v2 |");
        assert_eq!(csv_align("a,bbb\ncc,d"), "a  | bbb\ncc | d");
        assert_eq!(csv_transpose("1,2\n3,4"), "1,3\n2,4");
        assert_eq!(csv_sort_rows("k,v\nc,3\na,1\nb,2"), "k,v\na,1\nb,2\nc,3"); // 헤더 유지, 첫 열 정렬.
        assert_eq!(csv_to_sql("id,name\n1,Al"), "INSERT INTO t (id, name) VALUES ('1', 'Al');");
        assert!(crate::editordev2::json_to_toml("{\"a\":1}").contains("a = 1") && crate::editordev2::toml_to_json("a = 1").contains("\"a\": 1"));
    }
}
