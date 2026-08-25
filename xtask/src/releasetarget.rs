//! `xtask release-repo` — **릴리스를 어디에 올릴지 코드에서 읽어 준다.**
//!
//! ## 왜 필요한가 (2026-08-26에 실제로 당했다)
//!
//! 앱의 자동 업데이트는 `nabi-release/src/lib.rs`의 `REPO_PATH`가 가리키는 저장소 하나만
//! 본다. 그런데 배포 절차는 사람이 읽는 **문서**에 저장소 이름이 적혀 있었고, 2026-08-19에
//! 저장소를 통합하면서 코드만 바뀌고 문서는 옛 이름으로 남았다.
//!
//! 결과: v0.1.465부터 일곱 판이 **일몰 예정이던 저장소에만** 올라갔다. 앱이 묻는 주소는
//! 계속 v0.1.464를 돌려줬고, 그 사이 사용자는 "최신입니다"만 보며 **업데이트를 한 번도
//! 받지 못했다.** 릴리스는 매번 성공했으므로 아무 경고도 뜨지 않았다.
//!
//! 문서와 코드가 같은 사실을 두 번 적으면 언젠가 갈라진다. 이제 배포 절차는 이 명령이
//! 찍어 주는 값을 쓴다 — **코드가 유일한 출처다.**

use std::fs;
use std::process::ExitCode;

/// `REPO_PATH` 상수에서 `소유자/저장소`를 뽑는다.
///
/// 형식은 `/repos/<소유자>/<저장소>/releases/latest`. 모양이 달라지면 조용히 틀린 값을
/// 돌려주는 대신 실패한다 — 여기서 틀리면 또 엉뚱한 곳에 올라간다.
pub(crate) fn parse_repo(src: &str) -> Option<String> {
    let line = src.lines().find(|l| l.contains("REPO_PATH") && l.contains("/repos/"))?;
    let start = line.find("\"/repos/")? + "\"/repos/".len();
    let rest = &line[start..];
    let end = rest.find("/releases")?;
    let repo = &rest[..end];
    let ok = repo.split('/').count() == 2 && repo.split('/').all(|p| !p.is_empty());
    ok.then(|| repo.to_string())
}

pub(crate) fn run() -> ExitCode {
    let p = "crates/nabi-release/src/lib.rs";
    let Ok(src) = fs::read_to_string(p) else {
        eprintln!("{p}를 읽지 못했습니다");
        return ExitCode::FAILURE;
    };
    match parse_repo(&src) {
        Some(repo) => {
            println!("{repo}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("{p}에서 REPO_PATH를 해석하지 못했습니다 — 상수 모양이 바뀌었습니까?");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_repo;

    #[test]
    fn it_reads_the_repo_out_of_the_constant() {
        let src = "\npub(crate) const REPO_PATH: &str = \"/repos/acme/Widget/releases/latest\";\n";
        assert_eq!(parse_repo(src).as_deref(), Some("acme/Widget"));
    }

    /// **실제 소스에서 뽑힌다.** 이 시험이 배포 절차와 코드를 묶어 주는 매듭이다.
    #[test]
    fn the_real_source_still_parses() {
        let src = std::fs::read_to_string("../crates/nabi-release/src/lib.rs")
            .or_else(|_| std::fs::read_to_string("crates/nabi-release/src/lib.rs"))
            .expect("nabi-release/src/lib.rs를 찾지 못했습니다");
        let repo = parse_repo(&src).expect("REPO_PATH를 해석하지 못했습니다");
        assert!(repo.contains('/'), "{repo}");
        assert!(!repo.ends_with("Pub"), "일몰된 저장소를 가리키고 있습니다: {repo}");
    }

    /// 모양이 어긋나면 **틀린 값을 돌려주느니 실패한다.**
    #[test]
    fn a_broken_shape_is_refused() {
        assert_eq!(parse_repo("const REPO_PATH: &str = \"/repos/onlyone/releases/latest\";"), None);
        assert_eq!(parse_repo("아무 관계 없는 줄"), None);
    }
}
