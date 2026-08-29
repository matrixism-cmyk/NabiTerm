//! 소스에서 쓰는 i18n 키가 **실제로 카탈로그에 있는지** 대조한다.
//!
//! ## 왜 필요한가
//!
//! 모르는 키를 물으면 카탈로그는 `"?"` 한 글자를 돌려준다(`catalog.rs::tr` 의 마지막 줄).
//! 그래서 메뉴 이름에 오타가 하나 있으면 **화면에 `?` 한 글자짜리 단추**가 뜬다. 누를 수는
//! 있는데 무엇인지 알 수 없는 단추다. 컴파일도 되고 시험도 통과한다 — 아무도 안 잡는다.
//!
//! 키가 2,143개고 쓰는 자리가 1,193군데다. 눈으로 지킬 수 있는 규모가 아니다.
//!
//! ## 세는 방법과 함정
//!
//! **카탈로그 항목이 한 줄이라고 가정하면 안 된다.** 긴 항목은 rustfmt 가 줄을 나눠 놓아
//! 키가 홀로 한 줄을 차지한다. 처음에 한 줄만 보는 방식으로 만들었다가 멀쩡한 키 여든여섯
//! 개를 "없다"고 보고했다(2026-08-30). 그래서 공백을 눌러 붙인 뒤에 찾는다.
//!
//! 소스 쪽은 `tr(lang, "키")` 와 `trc("키")` 처럼 **글월로 적은 것만** 센다. 변수로 넘기는
//! 자리는 셀 수 없다 — 그런 자리는 이 검사가 지켜 주지 못한다는 뜻이라, 새 키는 되도록
//! 글월로 적는 편이 낫다.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

pub fn run() -> ExitCode {
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("작업 폴더를 알 수 없다");
        return ExitCode::FAILURE;
    };
    let root = cwd.join("crates");
    let mut catalog = String::new();
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let Ok(s) = std::fs::read_to_string(&p) else { continue };
            let shown = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
            match shown.starts_with("nabi-i18n") {
                true => catalog.push_str(&s),
                false => {
                    for k in used_keys(&s) {
                        used.entry(k).or_insert_with(|| shown.clone());
                    }
                }
            }
        }
    }

    let have = catalog_keys(&catalog);
    let missing: Vec<_> = used.iter().filter(|(k, _)| !have.contains(*k)).collect();
    for (k, where_) in &missing {
        println!("warn: {k} — {where_} (카탈로그에 없다 → 화면에 ? 로 나온다)");
    }
    println!("카탈로그 {} · 쓰는 키 {} · 없는 키 {}", have.len(), used.len(), missing.len());
    // 이것은 **경고가 아니라 실패**다. `?` 단추는 그 자리에서 쓸 수 없는 기능이 된다.
    match missing.is_empty() {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    }
}

/// 카탈로그의 키 — `("키", "en", …)` 의 첫 글월.
///
/// **여는 괄호와 따옴표 사이에 공백이 있을 수 있다.** rustfmt 가 긴 항목을 줄로 나누면
/// `(\n    "키",` 가 되고, 공백을 눌러 붙이면 `( "키",` 가 된다. 그 공백을 없애지 않으면
/// 그런 항목을 통째로 놓친다(실제로 여든한 개를 놓쳤다).
fn catalog_keys(src: &str) -> BTreeSet<String> {
    let flat = squeeze(src).replace("( \"", "(\"");
    let mut out = BTreeSet::new();
    for part in flat.split("(\"").skip(1) {
        let Some((key, rest)) = part.split_once('"') else { continue };
        // 다음이 `, "` 여야 카탈로그 항목이다 — 그냥 괄호 안의 글월과 구별한다.
        if rest.trim_start().starts_with(", \"") && is_key(key) {
            out.insert(key.to_string());
        }
    }
    out
}

/// 소스에서 `tr(…, "키")` / `trc("키")` 로 적은 키들.
///
/// **낱말 경계를 지켜야 한다.** 그냥 `tr(` 를 찾으면 `shell_from_str(` 안에도 걸려서
/// 아무 글월이나 키로 잡힌다(실제로 `"zzz"` 가 잡혔다).
fn used_keys(src: &str) -> Vec<String> {
    let flat = squeeze(src);
    let bytes = flat.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = flat[i..].find("tr") {
        let at = i + off;
        i = at + 2;
        // 앞이 글자면 다른 이름의 꼬리다(`str`, `attr` …).
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        // `tr(` 또는 `trc(` 만 본다.
        let rest = &flat[at + 2..];
        let Some(args) = rest.strip_prefix('(').or_else(|| rest.strip_prefix("c(")) else {
            continue;
        };
        let Some(args) = args.split(')').next() else { continue };
        let mut it = args.split('"');
        let (_, Some(k)) = (it.next(), it.next()) else { continue };
        if is_key(k) && !args.contains('{') {
            out.push(k.to_string());
        }
    }
    out
}

/// 키처럼 생겼는가 — 소문자·숫자·점·밑줄·붙임표만.
fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
}

/// 이어진 공백을 하나로 — 여러 줄에 걸친 항목을 한 줄처럼 본다.
fn squeeze(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        match c.is_whitespace() {
            true => {
                if !space {
                    out.push(' ');
                }
                space = true;
            }
            false => {
                out.push(c);
                space = false;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **여러 줄로 나뉜 항목도 찾아야 한다** — 이걸 놓쳐서 멀쩡한 키 여든여섯 개를
    /// 없다고 보고했다.
    #[test]
    fn 줄이_나뉜_항목도_찾는다() {
        let src = "    (\"a.b\", \"A\", \"가\", \"あ\"),\n    (\n        \"c.d\",\n        \"C\",\n        \"다\",\n        \"だ\",\n    ),\n";
        let keys = catalog_keys(src);
        assert!(keys.contains("a.b") && keys.contains("c.d"), "{keys:?}");
    }

    /// `shell_from_str(` 처럼 `tr` 로 끝나는 이름 안은 세지 않는다.
    #[test]
    fn 다른_이름의_꼬리는_세지_않는다() {
        let src = "shell_from_str(\"zzz\"); attr(\"x.y\");";
        assert!(used_keys(src).is_empty(), "{:?}", used_keys(src));
    }

    #[test]
    fn 쓰는_키를_뽑는다() {
        let src = "ui.button(tr(lang, \"menu.file\"));\nlet s = trc(\"err.gone\");\n";
        let mut got = used_keys(src);
        got.sort();
        got.dedup();
        assert!(got.contains(&"menu.file".to_string()), "{got:?}");
        assert!(got.contains(&"err.gone".to_string()), "{got:?}");
    }

    /// 키가 아닌 글월은 세지 않는다 — 서식 글월이 키로 잡히면 없는 키가 쏟아진다.
    #[test]
    fn 서식_글월은_키가_아니다() {
        assert!(!is_key("Save Workspace"));
        assert!(!is_key("{}/{}"));
        assert!(is_key("menu.file"));
    }
}
