//! 소스 파일 라인 수 게이트. 소프트 250(경고)/하드 400(실패).
//!
//! tokei 의존 없이 순수 std로 .rs 파일의 **코드 라인**만 센다(빈 줄·주석 제외).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SOFT: usize = 250;
const HARD: usize = 400;
/// 한 줄 최대 길이(문자). 줄 수 한도를 한 줄에 여러 문장을 몰아넣어 우회하는 것을 막는다
/// — 실제로 2천 자짜리 한 줄이 있었다. 줄 수만 세면 규율의 취지가 무력화된다.
const MAX_COLS: usize = 220;

pub fn run() -> ExitCode {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rs(&root.join("crates"), &mut files);
    collect_rs(&root.join("xtask").join("src"), &mut files);

    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    let mut long = Vec::new();
    for f in &files {
        let n = count_lines(f);
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if crate::overrides::is_allowed(&rel) {
            continue;
        }
        if let Some((ln, cols)) = widest_line(f) {
            if cols > MAX_COLS {
                long.push((rel.clone(), ln, cols));
            }
        }
        if n > HARD {
            failures.push((rel, n));
        } else if n > SOFT {
            warnings.push((rel, n));
        }
    }

    for (f, n) in &warnings {
        println!("warn: {f} = {n} 줄 (소프트 {SOFT} 초과)");
    }
    for (f, ln, cols) in &long {
        println!("warn: {f}:{ln} = {cols}자 (한 줄 {MAX_COLS}자 초과)");
    }
    for (f, n) in &failures {
        eprintln!("FAIL: {f} = {n} 줄 (하드 {HARD} 초과)");
    }
    println!(
        "검사 {} 파일 · 경고 {} · 실패 {}",
        files.len(),
        warnings.len() + long.len(),
        failures.len()
    );

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// 가장 긴 줄의 (줄 번호, 코드 문자 수). 주석 전용 줄은 세지 않고,
/// 문자열 리터럴 **안쪽**도 세지 않는다 — 잡으려는 것은 "한 줄에 문장 여러 개"이지
/// 긴 문장(i18n 번역문·안내 문구)이 아니다.
fn widest_line(path: &Path) -> Option<(usize, usize)> {
    let s = std::fs::read_to_string(path).ok()?;
    s.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .map(|(i, l)| (i + 1, code_cols(l)))
        .max_by_key(|&(_, c)| c)
}

/// 문자열 리터럴 내용을 뺀 줄 길이. 이스케이프(`\"`)를 건너뛴다.
fn code_cols(line: &str) -> usize {
    let (mut n, mut in_str, mut esc) = (0usize, false, false);
    for c in line.chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
                n += 1; // 닫는 따옴표는 코드로 센다.
            }
            continue;
        }
        n += 1;
        if c == '"' {
            in_str = true;
        }
    }
    n
}

/// 코드 라인만 센다 — 빈 줄과 주석 전용 줄(`//`/`///`/`//!`, 블록 `/* */`)은 제외.
fn count_lines(path: &Path) -> usize {
    let Ok(s) = std::fs::read_to_string(path) else {
        return 0;
    };
    let mut count = 0;
    let mut in_block = false;
    for raw in s.lines() {
        let line = raw.trim();
        if in_block {
            if let Some(rest) = line.split_once("*/") {
                in_block = false;
                if !rest.1.trim().is_empty() {
                    count += 1; // `*/` 뒤에 코드가 있으면 카운트.
                }
            }
            continue; // 블록 주석 안의 줄은 제외.
        }
        if line.is_empty() || line.starts_with("//") {
            continue; // 빈 줄·라인 주석 제외.
        }
        if let Some(after) = line.strip_prefix("/*") {
            if !after.contains("*/") {
                in_block = true; // 여러 줄 블록 주석 시작.
            }
            continue;
        }
        count += 1;
    }
    count
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn workspace_root() -> PathBuf {
    // xtask는 <root>/xtask 에 있으므로 CARGO_MANIFEST_DIR의 부모가 워크스페이스 루트.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    Path::new(&manifest)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
