//! `unsafe` 감사 게이트 — 모든 unsafe 사용처에 **왜 안전한지**를 적게 강제한다.
//!
//! 주석을 한 번 달아 두는 것만으로는 오래 못 간다(새 FFI가 들어올 때마다 다시 샌다).
//! 그래서 규율을 검사로 바꾼다: unsafe가 나오는 줄 위 5줄 안에 `SAFETY:`가 없으면 실패.
//!
//! 5줄인 이유는 속성(`#[cfg(...)]`)이나 짧은 설명이 사이에 끼는 실제 코드 모양 때문이다.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// SAFETY 주석을 찾아볼 범위(unsafe 줄 기준 위쪽 줄 수).
const LOOKBACK: usize = 5;

pub fn run() -> ExitCode {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rs(&root.join("crates"), &mut files);
    collect_rs(&root.join("xtask").join("src"), &mut files);

    let mut missing: Vec<(String, usize, String)> = Vec::new();
    let mut total = 0usize;
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if !has_unsafe(l) {
                continue;
            }
            total += 1;
            let from = i.saturating_sub(LOOKBACK);
            // **우리는 근거를 한국어로 적는다.** `SAFETY` 만 찾았더니 `// 안전:` 으로
            // 적어 둔 58곳을 "근거 없음"으로 보고했다 — 검사기가 틀린 것이었다.
            // 두 표기를 다 받는다. 새로 적을 때는 어느 쪽이든 뜻이 통하면 된다.
            // `unsafe fn` 은 `# Safety` 문서 주석으로 조건을 적는 것이 러스트 관례다.
            // 그것도 근거다 — 안 받으면 제대로 적어 둔 곳을 "없음"으로 보고한다.
            if lines[from..=i]
                .iter()
                .any(|p| p.contains("SAFETY") || p.contains("안전:") || p.contains("# Safety"))
            {
                continue;
            }
            let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
            missing.push((rel, i + 1, l.trim().chars().take(70).collect()));
        }
    }
    for (f, ln, src) in &missing {
        println!("fail: {f}:{ln} — SAFETY 주석 없음  ({src})");
    }
    println!("unsafe {total}곳 · 근거 주석 없음 {}곳", missing.len());
    if missing.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// 이 줄이 실제 unsafe 사용인가 — 주석과 **문자열 리터럴 안의 단어는 제외**한다.
/// (검사기 자신의 테스트 문자열에 걸려 게이트가 스스로 실패하는 일이 실제로 있었다.)
fn has_unsafe(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with('*') {
        return false;
    }
    let code = t.split("//").next().unwrap_or(t); // 줄 끝 주석 제거.
    strip_strings(code)
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| w == "unsafe")
}

/// 큰따옴표 문자열 리터럴의 내용을 지운다(이스케이프 `\"`는 문자열 안으로 본다).
fn strip_strings(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut in_str = false;
    let mut esc = false;
    for c in code.chars() {
        match (in_str, c) {
            (false, '"') => in_str = true,
            (false, _) => out.push(c),
            (true, _) if esc => esc = false,
            (true, '\\') => esc = true,
            (true, '"') => in_str = false,
            (true, _) => {}
        }
    }
    out
}

fn workspace_root() -> PathBuf {
    let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    while !p.join("Cargo.lock").exists() {
        if !p.pop() {
            break;
        }
    }
    p
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "vendor") {
                continue; // 벤더 코드는 우리 규율 대상이 아니다.
            }
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::has_unsafe;

    #[test]
    fn detects_real_unsafe_only() {
        assert!(has_unsafe("    unsafe {"));
        assert!(has_unsafe("let x = unsafe { f() };"));
        assert!(!has_unsafe("// unsafe 설명 주석"));
        assert!(!has_unsafe("/// unsafe 문서 주석"));
        assert!(!has_unsafe("let unsafely = 1;")); // 단어 경계.
    }

    /// 문자열 리터럴 안의 단어는 코드가 아니다 — 이걸 못 걸러 게이트가 스스로 실패했다.
    #[test]
    fn ignores_string_literals() {
        let quoted = format!("{}unsafe-audit{} => run(),", '"', '"');
        assert!(!has_unsafe(&quoted));
        let asserted = format!("assert!(has_unsafe({}    unsafe {{{}));", '"', '"');
        assert!(!has_unsafe(&asserted));
        // 문자열이 앞에 있어도 뒤의 진짜 unsafe는 잡는다.
        let mixed = format!("log({}msg{}); unsafe {{ g() }}", '"', '"');
        assert!(has_unsafe(&mixed));
    }
}
