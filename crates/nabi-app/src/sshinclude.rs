//! **`Include`를 펼친다** — 요즘 `~/.ssh/config`는 대개 여러 파일로 쪼개져 있다.
//!
//! ```text
//!   Include config.d/*
//!   Include work/hosts
//! ```
//!
//! 파서는 글자만 보므로, 읽어 들이기 전에 **한 덩어리로 이어 붙여** 넘긴다. 이 층을
//! 따로 둔 까닭은 파서를 파일 시스템에서 떼어 놓기 위해서다 — 파서는 지금도 글자만 받고,
//! 시험도 글자로만 한다.
//!
//! ## 조심한 것
//!
//! * **고리**(a가 b를, b가 다시 a를 포함) — 본 파일은 다시 읽지 않는다. 안 그러면 멈추지 않는다.
//! * **깊이** — 다섯 겹에서 끊는다. 사람이 쓰는 설정에 그보다 깊은 것은 없다.
//! * **경로** — 상대 경로는 `~/.ssh` 기준이다(OpenSSH가 그렇게 읽는다).
//! * **없는 파일** — 조용히 넘어간다. OpenSSH도 `Include`에 안 맞는 것을 오류로 삼지 않는다.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 최대 깊이. 사람이 쓰는 설정은 두세 겹을 넘지 않는다.
const MAX_DEPTH: usize = 5;

/// `Include` 줄을 실제 내용으로 바꾼 글자를 돌려준다.
pub(crate) fn expand(content: &str, base: &Path) -> String {
    let mut seen = HashSet::new();
    expand_inner(content, base, 0, &mut seen)
}

fn expand_inner(content: &str, base: &Path, depth: usize, seen: &mut HashSet<PathBuf>) -> String {
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        let Some(pat) = include_arg(line) else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        if depth >= MAX_DEPTH {
            continue; // 너무 깊다 — 조용히 멈춘다(설정이 깨진 것이지 우리 잘못이 아니다).
        }
        for p in resolve(&pat, base) {
            // 한 번 읽은 파일은 다시 읽지 않는다 — 고리를 끊는 유일한 장치다.
            if !seen.insert(p.clone()) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&p) {
                out.push_str(&expand_inner(&text, base, depth + 1, seen));
                out.push('\n');
            }
        }
    }
    out
}

/// 이 줄이 `Include`면 인자를, 아니면 None. OpenSSH는 `Key=Value`도 받는다.
pub(crate) fn include_arg(line: &str) -> Option<String> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let at = t.find(|c: char| c.is_whitespace() || c == '=')?;
    if !t[..at].eq_ignore_ascii_case("include") {
        return None;
    }
    let v = t[at..].trim_start_matches([' ', '\t', '=']).trim().trim_matches('"');
    (!v.is_empty()).then(|| v.to_string())
}

/// 한 `Include` 인자(여러 개일 수 있고 별표를 쓸 수 있다)를 실제 파일 목록으로.
fn resolve(arg: &str, base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for token in arg.split_whitespace() {
        let full = match Path::new(token).is_absolute() {
            true => PathBuf::from(token),
            // 상대 경로는 ~/.ssh 기준 — OpenSSH가 그렇게 읽는다.
            false => base.join(token),
        };
        match full.to_string_lossy().contains('*') {
            false => out.push(full),
            true => out.extend(glob_dir(&full)),
        }
    }
    out.sort();
    out
}

/// 한 겹짜리 별표만 푼다. OpenSSH의 전체 glob은 아니지만 실제로 쓰이는 꼴이다.
fn glob_dir(pat: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (pat.parent(), pat.file_name()) else {
        return Vec::new();
    };
    let name = name.to_string_lossy();
    let Some((pre, post)) = name.split_once('*') else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.len() >= pre.len() + post.len() && n.starts_with(pre) && n.ends_with(post)
        })
        .map(|e| e.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{expand, include_arg};

    #[test]
    fn a_plain_line_is_not_an_include() {
        assert_eq!(include_arg("Host web"), None);
        assert_eq!(include_arg("# Include something"), None);
        assert_eq!(include_arg(""), None);
    }

    /// OpenSSH가 받는 꼴을 모두 받는다(대소문자·등호·따옴표·앞 공백).
    #[test]
    fn every_spelling_openssh_accepts_is_accepted() {
        for l in ["Include c.d", "  include   c.d", "INCLUDE=c.d", "Include \"c.d\""] {
            assert_eq!(include_arg(l).as_deref(), Some("c.d"), "{l}");
        }
    }

    #[test]
    fn an_include_without_a_path_is_ignored() {
        assert_eq!(include_arg("Include"), None);
        assert_eq!(include_arg("Include   "), None);
    }

    /// 파일이 없으면 **조용히 넘어간다** — OpenSSH도 그것을 오류로 삼지 않는다.
    #[test]
    fn a_missing_include_does_not_break_the_rest() {
        let dir = std::env::temp_dir().join("nabi_inc_missing");
        let got = expand("Host a\nInclude nope.conf\nHost b\n", &dir);
        assert!(got.contains("Host a") && got.contains("Host b"), "{got}");
        assert!(!got.contains("Include"), "펼치지 못한 줄이 그대로 남았다");
    }

    /// **진짜 파일을 이어 붙인다** — 이 기능의 전부다.
    #[test]
    fn an_included_file_is_pasted_in_place() {
        let dir = std::env::temp_dir().join("nabi_inc_ok");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("work.conf"), "Host inner\n  HostName 10.0.0.9\n").unwrap();
        let got = expand("Host outer\nInclude work.conf\n", &dir);
        assert!(got.contains("Host outer") && got.contains("Host inner"), "{got}");
        assert!(got.contains("10.0.0.9"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **고리에 빠지지 않는다** — 서로를 포함하는 두 파일에서 멈추지 않으면 프로그램이 선다.
    #[test]
    fn two_files_including_each_other_terminate() {
        let dir = std::env::temp_dir().join("nabi_inc_loop");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.conf"), "Host a\nInclude b.conf\n").unwrap();
        std::fs::write(dir.join("b.conf"), "Host b\nInclude a.conf\n").unwrap();
        let got = expand("Include a.conf\n", &dir);
        assert!(got.contains("Host a") && got.contains("Host b"), "{got}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 별표는 그 폴더의 맞는 파일들을 모두 가져온다(요즘 설정이 쓰는 꼴).
    #[test]
    fn a_star_pulls_in_every_matching_file() {
        let dir = std::env::temp_dir().join("nabi_inc_star");
        let d2 = dir.join("config.d");
        let _ = std::fs::create_dir_all(&d2);
        std::fs::write(d2.join("one.conf"), "Host one\n").unwrap();
        std::fs::write(d2.join("two.conf"), "Host two\n").unwrap();
        std::fs::write(d2.join("skip.txt"), "Host skip\n").unwrap();
        let got = expand("Include config.d/*.conf\n", &dir);
        assert!(got.contains("Host one") && got.contains("Host two"), "{got}");
        assert!(!got.contains("Host skip"), "맞지 않는 파일까지 가져왔다");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
