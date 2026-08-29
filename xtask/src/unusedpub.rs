//! 크레이트 경계를 넘는 `pub fn` 가운데 **아무도 쓰지 않는 것**을 찾는다.
//!
//! ## 왜 필요한가
//!
//! clippy 는 크레이트 **안**의 미사용만 잡는다. 라이브러리 크레이트가 내놓은 `pub` 은
//! 아무도 안 써도 아무 말이 없다. 그래서 "만들어 놓고 쓰지 않는" 것이 조용히 쌓인다.
//!
//! 2026-08-30 전수 점검에서 열셋이 나왔고, 그중 셋은 단순 미사용이 아니라 결함이었다.
//!
//! * `did_save` — 저장 통지를 안 보내서 rust-analyzer 가 `cargo check` 를 안 돌렸다.
//! * `load_reporting` — 어긋난 설정 키를 알려 주는 길이 있는데 아무도 안 봤다.
//! * `normalize_newlines` — 붙여넣기 줄바꿈을 안 맞춰 셸에 빈 줄이 하나씩 더 들어갔다.
//!
//! 손으로 한 번 찾는 것으로는 또 쌓인다. 그래서 검사로 만든다.
//!
//! ## 세는 방법
//!
//! 이름이 소스 전체에 몇 번 나오는지 센다. **부르는 자리만 세면 안 된다** — 메뉴 표처럼
//! 함수를 값으로 넘기는 곳을 놓쳐 멀쩡한 기능을 미사용으로 잘못 보고한다(실제로 겪었다).
//! 시험 안의 쓰임도 치지 않는다. 시험만 부르는 것은 아무도 안 쓰는 것과 같다.
//!
//! ## 왜 실패가 아니라 경고인가
//!
//! 곧 쓸 작정으로 먼저 내놓는 일이 있고, 이름만으로는 판단할 수 없다. 세어서 보여 주되
//! 막지는 않는다 — 막으면 목록에 예외를 적기 시작하고, 예외 목록은 곧 낡는다.

use std::collections::HashMap;
use std::process::ExitCode;
use std::path::{Path, PathBuf};

/// 자료형만 있는 크레이트 — 여기 pub 은 쓰이지 않아도 이상하지 않다.
const SKIP_CRATES: [&str; 3] = ["nabi-types", "nabi-proto", "nabi-error"];
/// 흔한 관례 이름 — 트레이트 구현이라 이름만으로는 셀 수 없다.
const SKIP_NAMES: [&str; 8] =
    ["new", "default", "fmt", "clone", "drop", "from", "len", "is_empty"];

pub fn run() -> ExitCode {
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("작업 폴더를 알 수 없다");
        return ExitCode::FAILURE;
    };
    let root = cwd.join("crates");
    let mut files: Vec<(String, String)> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    files.push((p.display().to_string(), s));
                }
            }
        }
    }

    // 시험을 뺀 본문만 합쳐 둔다 — 여기서 이름을 센다.
    let body: String =
        files.iter().map(|(_, s)| strip_tests(s)).collect::<Vec<_>>().join("\n");

    let mut defs: HashMap<String, String> = HashMap::new();
    for (path, s) in &files {
        if skip_file(path) {
            continue;
        }
        for name in pub_fns(strip_tests(s)) {
            defs.entry(name).or_insert_with(|| path.clone());
        }
    }

    let mut unused: Vec<(String, String)> = defs
        .into_iter()
        .filter(|(n, _)| !SKIP_NAMES.contains(&n.as_str()))
        .filter(|(n, _)| count_word(&body, n) <= 1)
        .collect();
    unused.sort();

    for (name, path) in &unused {
        let short = PathBuf::from(path);
        let short = short.strip_prefix(&root).unwrap_or(&short);
        println!("warn: {name} — {} (아무도 쓰지 않는다)", short.display());
    }
    println!("검사 {} 파일 · 안 쓰는 pub fn {}", files.len(), unused.len());
    ExitCode::SUCCESS
}

/// nabi-app(바이너리)과 자료형 크레이트는 세지 않는다.
fn skip_file(path: &str) -> bool {
    let p = Path::new(path);
    p.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == "nabi-app" || SKIP_CRATES.contains(&s.as_ref())
    })
}

/// `#[cfg(test)]` 앞까지만 — 시험 안의 쓰임은 쓰임이 아니다.
fn strip_tests(s: &str) -> &str {
    match s.find("#[cfg(test)]") {
        Some(i) => &s[..i],
        None => s,
    }
}

/// 그 파일이 내놓는 `pub fn` 이름들.
fn pub_fns(s: &str) -> Vec<String> {
    s.lines()
        .filter_map(|l| {
            let t = l.trim_start();
            let rest = t.strip_prefix("pub fn ").or_else(|| t.strip_prefix("pub unsafe fn "))?;
            let name: String =
                rest.chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_').collect();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// 낱말 경계를 지켜 센다 — `hash` 가 `hash_of` 에 걸리면 안 된다.
fn count_word(hay: &str, word: &str) -> usize {
    let bytes = hay.as_bytes();
    let w = word.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut n = 0;
    let mut i = 0;
    while let Some(off) = hay[i..].find(word) {
        let at = i + off;
        let before_ok = at == 0 || !ident(bytes[at - 1]);
        let after = at + w.len();
        let after_ok = after >= bytes.len() || !ident(bytes[after]);
        if before_ok && after_ok {
            n += 1;
        }
        i = at + 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 낱말_경계를_지켜_센다() {
        assert_eq!(count_word("hash hash_of ahash", "hash"), 1);
        assert_eq!(count_word("fn eta_secs() eta_secs()", "eta_secs"), 2);
    }

    #[test]
    fn pub_fn_이름만_뽑는다() {
        let s = "pub fn alpha(x: u8) {}\n    pub unsafe fn beta() {}\nfn gamma() {}\n";
        assert_eq!(pub_fns(s), vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn 시험_뒤는_보지_않는다() {
        let s = "pub fn a() {}\n#[cfg(test)]\npub fn b() {}\n";
        assert_eq!(pub_fns(strip_tests(s)), vec!["a".to_string()]);
    }
}
