//! 사전배포(pre-release) 의존성 게이트 — 잠금 파일에 alpha/beta/rc가 섞이지 않았는지 본다.
//!
//! cargo-deny로는 잡을 수 없다(버전 **범위**는 보지만 잠긴 버전이 사전배포인지는 검사 항목이
//! 아니다). 사전배포 크레이트는 예고 없이 API가 바뀌고 yank되기도 해서, 배포물에 들어가면
//! 나중에 같은 커밋을 다시 빌드하지 못할 수 있다.
//!
//! 일부러 쓰는 경우가 있으므로 허용 목록을 둔다: `prerelease-allow.toml`에 이름 한 줄씩.

use std::path::Path;
use std::process::ExitCode;

/// 잠금 파일에서 찾은 사전배포 패키지.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub version: String,
}

/// semver 사전배포인가 — 빌드 메타데이터(`+`)는 사전배포가 아니다.
///
/// `1.0.0-rc.1`은 참, `1.0.0+build.5`는 거짓, `1.0.0-rc.1+b`는 참.
pub fn is_prerelease(version: &str) -> bool {
    let core = version.split('+').next().unwrap_or(version);
    core.split_once('-').is_some_and(|(_, pre)| !pre.is_empty())
}

/// `Cargo.lock` 본문에서 사전배포 패키지를 모은다(허용 목록에 있는 이름은 뺀다).
///
/// TOML 파서를 새로 들이지 않는다 — 잠금 파일 형식은 고정이라 `name =`/`version =` 두 줄만
/// 보면 충분하고, xtask에 의존성을 늘리지 않는 편이 빌드에도 낫다.
pub fn scan(lock: &str, allow: &[String]) -> Vec<Found> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in lock.lines() {
        let t = line.trim();
        if t == "[[package]]" {
            name = None;
        } else if let Some(v) = field(t, "name") {
            name = Some(v);
        } else if let Some(v) = field(t, "version") {
            if let Some(n) = name.take() {
                if is_prerelease(&v) && !allow.iter().any(|a| a == &n) {
                    out.push(Found { name: n, version: v });
                }
            }
        }
    }
    out
}

/// `key = "value"` 한 줄에서 값만 꺼낸다(아니면 None).
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// 허용 목록 파일을 읽는다(없으면 빈 목록). `#` 주석과 빈 줄은 무시.
pub fn load_allow(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn run() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let lock = match std::fs::read_to_string(root.join("Cargo.lock")) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Cargo.lock을 읽지 못했습니다: {e}");
            return ExitCode::FAILURE;
        }
    };
    let allow = load_allow(&root.join("prerelease-allow.toml"));
    let found = scan(&lock, &allow);
    if found.is_empty() {
        println!("사전배포 의존성 없음 (허용 목록 {}건)", allow.len());
        return ExitCode::SUCCESS;
    }
    eprintln!("사전배포 의존성 {}건 — 배포에 넣지 말 것:", found.len());
    for f in &found {
        eprintln!("  {} {}", f.name, f.version);
    }
    eprintln!("일부러 쓰는 것이면 prerelease-allow.toml에 이름을 적으세요.");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_semver_prerelease() {
        assert!(is_prerelease("1.0.0-rc.1"));
        assert!(is_prerelease("0.2.0-alpha"));
        assert!(is_prerelease("1.0.0-rc.1+build"));
        assert!(!is_prerelease("1.0.0"));
        assert!(!is_prerelease("0.61.2"));
        // 빌드 메타데이터는 사전배포가 아니다 — 하이픈이 `+` 뒤에 있어도 속으면 안 된다.
        assert!(!is_prerelease("1.0.0+build-5"));
    }

    fn lock() -> &'static str {
        r#"
[[package]]
name = "stable-one"
version = "1.2.3"

[[package]]
name = "risky"
version = "2.0.0-beta.4"

[[package]]
name = "known-pre"
version = "0.1.0-rc.1"
"#
    }

    #[test]
    fn finds_only_prereleases() {
        let f = scan(lock(), &[]);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0], Found { name: "risky".into(), version: "2.0.0-beta.4".into() });
    }

    /// 일부러 쓰는 크레이트는 허용 목록으로 뺀다(게이트가 통째로 무력해지지 않게 이름 단위).
    #[test]
    fn allowlist_excludes_by_name() {
        let f = scan(lock(), &["known-pre".to_string()]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "risky");
    }

    /// 패키지 블록이 아닌 곳의 name/version 짝에 속지 않는다.
    #[test]
    fn ignores_fields_outside_package_blocks() {
        let s = "[metadata]\nname = \"x\"\n\n[[package]]\nname = \"a\"\nversion = \"1.0.0\"\n";
        assert!(scan(s, &[]).is_empty());
    }

    #[test]
    fn allowlist_skips_comments_and_blanks() {
        let dir = std::env::temp_dir().join(format!("nabi-allow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("prerelease-allow.toml");
        std::fs::write(&p, "# 주석\n\n  risky  \n").unwrap();
        assert_eq!(load_allow(&p), vec!["risky".to_string()]);
        let _ = std::fs::remove_file(&p);
    }

    /// 파일이 없으면 빈 목록 — 게이트가 파일 부재로 죽지 않는다.
    #[test]
    fn missing_allowlist_is_empty() {
        assert!(load_allow(Path::new("does-not-exist.toml")).is_empty());
    }
}
