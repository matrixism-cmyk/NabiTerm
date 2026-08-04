//! 소스 파일 라인 수 게이트. 소프트 250(경고)/하드 400(실패).
//!
//! tokei 의존 없이 순수 std로 .rs 파일의 **코드 라인**만 센다(빈 줄·주석 제외).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SOFT: usize = 250;
const HARD: usize = 400;

pub fn run() -> ExitCode {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rs(&root.join("crates"), &mut files);
    collect_rs(&root.join("xtask").join("src"), &mut files);

    let mut warnings = Vec::new();
    let mut failures = Vec::new();
    for f in &files {
        let n = count_lines(f);
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if crate::overrides::is_allowed(&rel) {
            continue;
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
    for (f, n) in &failures {
        eprintln!("FAIL: {f} = {n} 줄 (하드 {HARD} 초과)");
    }
    println!(
        "검사 {} 파일 · 경고 {} · 실패 {}",
        files.len(),
        warnings.len(),
        failures.len()
    );

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
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
