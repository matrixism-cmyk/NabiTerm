//! 설정 키 전수 점검 — **적어 두기만 하고 아무도 안 보는 설정**을 찾는다.
//!
//! ## 왜 필요한가
//!
//! 설정 필드가 98개다. 필드를 하나 더하고 저장·불러오기까지 붙여 놓고 **정작 읽는 자리를
//! 안 붙이는 일**이 생긴다. 그러면 사용자는 설정을 바꾸고 아무 일도 일어나지 않는 것을
//! 본다 — 고장 났다고 느끼지만 어디에도 오류가 없다.
//!
//! 컴파일러는 못 잡는다. 필드는 직렬화에 쓰이므로 "쓰이지 않는다"고 말하지 않는다.
//! 메뉴는 배치 BE 에서, 크레이트 밖 미사용은 배치 BD 에서 이렇게 셌다. 설정은 남아 있었다.
//!
//! ## 두 가지를 센다
//!
//! 1. **읽는 자리가 있는가** — `nabi-config` 밖에서 그 이름을 쓰는 곳.
//! 2. **바꿀 수 있는가** — 어딘가에서 값을 넣거나(`= `) 만질 수 있게 넘기는가(`&mut`).
//!
//! ⚠️ 처음에는 둘째를 "`settings*.rs` 안에 나오는가"로 셌다. 틀렸다 — 설정 UI 는 그
//! 파일들 밖에도 있다(AI CLI 자동 갱신은 환경 관리자, 볼트 기억은 볼트 창). 그래서
//! 멀쩡한 것 둘을 "화면에 없다"고 보고했다. **바꿀 수 있는가**로 세면 파일 이름과
//! 무관하게 맞는다.
//!
//! 둘째가 없어도 결함이 아닐 수 있다(앱이 스스로 적는 기억값은 사람이 안 바꾼다).
//! 그래도 `ssh_stats_secs` 처럼 **문서에 "기본 3, 0=끄기"라고 적어 둔 조절값**이 화면에
//! 없으면 그것은 결함이다 — 실제로 이 검사로 찾았다.
//!
//! ## 왜 실패가 아니라 경고인가
//!
//! `unused` 와 같은 이유다. 곧 쓸 작정으로 먼저 넣는 일이 있고, 이름만으로는 판단할 수
//! 없다. 세어서 보여 주되 막지 않는다 — 막으면 예외 목록이 생기고, 예외 목록은 곧 낡는다.

use std::path::Path;
use std::process::ExitCode;

/// 이름이 흔해 세면 거짓 양성이 나는 것들 — 다른 뜻으로 쓰이는 낱말이다.
const SKIP: [&str; 4] = ["name", "path", "kind", "value"];

pub fn run() -> ExitCode {
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("작업 폴더를 알 수 없다");
        return ExitCode::FAILURE;
    };
    let schema = cwd.join("crates/nabi-config/src/schema.rs");
    let Ok(src) = std::fs::read_to_string(&schema) else {
        eprintln!("설정 스키마를 읽지 못했다: {}", schema.display());
        return ExitCode::FAILURE;
    };
    let fields = fields_of(&src);
    if fields.is_empty() {
        eprintln!("스키마에서 필드를 하나도 못 찾았다 — 검사기가 틀렸다");
        return ExitCode::FAILURE;
    }

    let files = rust_files(&cwd.join("crates"));
    // `nabi-config` 자신은 빼고 센다 — 거기서는 저장·불러오기에 늘 나온다.
    // 읽는 자리와 바꾸는 자리를 같은 글에서 센다(설정 UI 는 파일 이름으로 가릴 수 없다).
    let outside: String = files
        .iter()
        .filter(|(p, _)| !p.contains("nabi-config"))
        .map(|(_, s)| strip_tests(s))
        .collect();
    let (mut unread, mut offscreen) = (Vec::new(), Vec::new());
    for f in &fields {
        if SKIP.contains(&f.as_str()) {
            continue;
        }
        if !mentions(&outside, f) {
            unread.push(f.clone());
        } else if !writable(&outside, f) {
            offscreen.push(f.clone());
        }
    }

    for f in &unread {
        println!("warn: {f} — 설정에 있는데 **아무도 읽지 않는다**");
    }
    for f in &offscreen {
        println!("note: {f} — 읽기만 하고 **아무 데서도 못 바꾼다**(파일을 직접 고쳐야 한다)");
    }
    println!(
        "설정 필드 {} · 안 읽는 것 {} · 못 바꾸는 것 {}",
        fields.len(),
        unread.len(),
        offscreen.len()
    );
    ExitCode::SUCCESS
}

/// 스키마에서 필드 이름을 뽑는다.
///
/// `#[serde(...)]` 줄과 주석은 건너뛰고 `pub <이름>: <형>` 만 본다.
fn fields_of(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("pub ") else { continue };
        let Some((name, _)) = rest.split_once(':') else { continue };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            continue;
        }
        if !out.contains(&name.to_string()) {
            out.push(name.to_string());
        }
    }
    out
}

/// 그 이름이 **낱말로** 나오는가.
///
/// 낱말 경계를 지키지 않으면 `log` 가 `catalog` 안에 걸린다 — 배치 BE 에서 실제로
/// 검사기가 그렇게 틀렸다.
fn mentions(hay: &str, word: &str) -> bool {
    let b = hay.as_bytes();
    let w = word.as_bytes();
    let mut i = 0;
    while let Some(p) = find_from(b, w, i) {
        let before_ok = p == 0 || !is_word(b[p - 1]);
        let after = p + w.len();
        let after_ok = after >= b.len() || !is_word(b[after]);
        if before_ok && after_ok {
            return true;
        }
        i = p + 1;
    }
    false
}

/// 그 이름에 **값을 넣거나 만질 수 있게 넘기는** 자리가 있는가.
///
/// 세 가지를 본다.
/// * `이름 = ` — 값을 넣는다(`==` 는 견주는 것이라 뺀다).
/// * 바로 앞 표현식에 `&mut` — egui 의 체크상자·슬라이더가 이렇게 받는다.
/// * `이름.push(` 같은 **바꾸는 메서드** — 목록·표는 통째로 넣지 않고 이렇게 고친다.
///
/// ⚠️ 셋째를 빠뜨렸더니 멀쩡한 것 여섯을 "못 바꾼다"고 보고했다(북마크·세션 메모·
/// 링크 규칙 등). 목록형 설정은 거의 다 이 형태다.
fn writable(hay: &str, word: &str) -> bool {
    let b = hay.as_bytes();
    let w = word.as_bytes();
    let mut i = 0;
    while let Some(p) = find_from(b, w, i) {
        i = p + 1;
        let before_ok = p == 0 || !is_word(b[p - 1]);
        let after = p + w.len();
        if !before_ok || (after < b.len() && is_word(b[after])) {
            continue;
        }
        // 값을 넣는가.
        let mut j = after;
        while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' && b.get(j + 1) != Some(&b'=') {
            return true;
        }
        // 바꾸는 메서드를 부르는가.
        if b.get(after) == Some(&b'.') && starts_with_mutating(&b[after + 1..]) {
            return true;
        }
        // 만질 수 있게 넘기는가 — 앞쪽 같은 표현식 안에 `&mut` 가 있는가.
        //
        // ⚠️ 바이트로 다룬다. 글자 단위로 자르면 한글 주석 한가운데를 잘라 터진다
        // (실제로 터졌다 — 소스에는 한글 주석이 가득하다).
        let start = p.saturating_sub(80);
        let head = &b[start..p];
        // 표현식 경계 뒤만 본다(세미콜론·줄바꿈 뒤).
        let from = head
            .iter()
            .rposition(|c| *c == b';' || *c == b'\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        if find_from(&head[from..], b"&mut", 0).is_some() {
            return true;
        }
    }
    false
}

/// 목록·표를 **고치는** 메서드 이름으로 시작하는가.
fn starts_with_mutating(rest: &[u8]) -> bool {
    const M: [&str; 13] = [
        "push", "insert", "remove", "retain", "clear", "pop", "extend", "iter_mut", "get_mut",
        "entry", "truncate", "sort", "dedup",
    ];
    M.iter().any(|m| rest.starts_with(m.as_bytes()))
}

fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `#[cfg(test)]` 뒤는 뺀다 — 시험만 읽는 설정은 아무도 안 읽는 것과 같다.
fn strip_tests(s: &str) -> String {
    match s.find("#[cfg(test)]") {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

fn rust_files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    out.push((p.display().to_string(), s));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 스키마에서_필드를_뽑는다() {
        let src = "pub struct A {\n    /// 설명\n    #[serde(default)]\n    pub font_size: f32,\n    pub Bad: u8,\n}\n";
        assert_eq!(fields_of(src), vec!["font_size".to_string()]);
    }

    /// 낱말 경계를 안 지키면 `log` 가 `catalog` 안에 걸린다 — 실제로 겪은 실수다.
    #[test]
    fn 낱말_경계를_지킨다() {
        assert!(!mentions("let catalog = 1;", "log"));
        assert!(mentions("cfg.terminal.log = 1;", "log"));
        assert!(!mentions("logger()", "log"));
    }

    #[test]
    fn 바꾸는_자리를_알아본다() {
        assert!(writable("cfg.terminal.scrollback = 5000;", "scrollback"));
        assert!(writable("chk(ui, label, &mut cfg.terminal.confirm_close);", "confirm_close"));
        // 견주기만 하는 것은 바꾸는 것이 아니다.
        assert!(!writable("if cfg.terminal.scrollback == 0 { }", "scrollback"));
        // 읽기만 하는 것도 아니다.
        assert!(!writable("let n = cfg.terminal.scrollback;", "scrollback"));
        // 앞 표현식이 끝났으면 그 `&mut` 는 남의 것이다.
        assert!(!writable("f(&mut a);\n let n = cfg.x.scrollback;", "scrollback"));
        // 목록은 통째로 넣지 않고 고친다 — 이것도 바꾸는 것이다.
        assert!(writable("cfg.terminal.sftp_bookmarks.push(p);", "sftp_bookmarks"));
        assert!(writable("cfg.terminal.last_connected.insert(k, v);", "last_connected"));
        // 읽기만 하는 메서드는 아니다.
        assert!(!writable("cfg.terminal.sftp_bookmarks.contains(&p)", "sftp_bookmarks"));
        assert!(!writable("cfg.terminal.sftp_bookmarks.clone()", "sftp_bookmarks"));
    }

    #[test]
    fn 시험_뒤는_세지_않는다() {
        let s = "fn a() { x }\n#[cfg(test)]\nmod t { fn b() { secret_key } }\n";
        assert!(!strip_tests(s).contains("secret_key"));
    }
}
