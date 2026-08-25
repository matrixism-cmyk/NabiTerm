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

/// 옛 클라이언트가 묻는 저장소 — **여기에도 계속 올려야 한다.**
///
/// v0.1.446 이하로 설치된 앱은 이 저장소를 묻도록 컴파일돼 있다. 새 릴리스가 여기 없으면
/// 그 사용자들은 스스로 넘어올 방법이 없어 영원히 갇힌다. 그래서 이 목록은 "전환기 임시
/// 조치"가 아니라 **그 버전들이 사라질 때까지 계속되는 약속**이다.
///
/// 이 값이 앱 런타임에 쓰이지 않는 것은 맞다 — 옛 버전에만 박혀 있는 사실이라 지금 코드가
/// 참조할 곳이 없다. 그래서 **배포 절차를 담당하는 xtask가 들고 있는다.**
const LEGACY_MIRRORS: &[&str] = &["matrixism-cmyk/NabiTermPub"];

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

/// 릴리스를 올려야 할 곳 전부 — 첫 줄이 앱이 묻는 곳, 나머지는 옛 클라이언트용 거울.
pub(crate) fn all_targets(src: &str) -> Option<Vec<String>> {
    let mut v = vec![parse_repo(src)?];
    v.extend(LEGACY_MIRRORS.iter().map(|s| s.to_string()));
    Some(v)
}

/// `--all`이면 거울까지, 아니면 앱이 묻는 곳 하나만 찍는다(스크립트가 그대로 받아 쓴다).
pub(crate) fn run() -> ExitCode {
    let p = "crates/nabi-release/src/lib.rs";
    let Ok(src) = fs::read_to_string(p) else {
        eprintln!("{p}를 읽지 못했습니다");
        return ExitCode::FAILURE;
    };
    let Some(repo) = parse_repo(&src) else {
        eprintln!("{p}에서 REPO_PATH를 해석하지 못했습니다 — 상수 모양이 바뀌었습니까?");
        return ExitCode::FAILURE;
    };
    if std::env::args().any(|a| a == "--all") {
        for r in all_targets(&src).unwrap_or_default() {
            println!("{r}");
        }
    } else {
        println!("{repo}");
    }
    ExitCode::SUCCESS
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

    /// **옛 클라이언트가 묻는 곳이 목록에서 빠지면 안 된다.** 빠뜨리면 그 사용자들은
    /// 새 릴리스를 영영 못 보고, 릴리스는 매번 성공하므로 아무 경고도 뜨지 않는다.
    #[test]
    fn the_legacy_mirror_is_included_alongside_the_primary() {
        let src = "pub(crate) const REPO_PATH: &str = \"/repos/acme/Widget/releases/latest\";";
        let t = super::all_targets(src).expect("목록을 만들지 못했습니다");
        assert_eq!(t[0], "acme/Widget", "첫 줄은 앱이 묻는 곳이어야 한다");
        assert!(t.iter().any(|r| r.ends_with("NabiTermPub")), "{t:?}");
        assert!(t.len() >= 2);
    }

    /// 모양이 어긋나면 **틀린 값을 돌려주느니 실패한다.**
    #[test]
    fn a_broken_shape_is_refused() {
        assert_eq!(parse_repo("const REPO_PATH: &str = \"/repos/onlyone/releases/latest\";"), None);
        assert_eq!(parse_repo("아무 관계 없는 줄"), None);
    }
}
