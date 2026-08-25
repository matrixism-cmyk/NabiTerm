//! 업데이트 뒤 **"새로워진 점"** — 무엇이 바뀌었는지 설치하고 나서 볼 수 있게.
//!
//! 지금까지 릴리스 노트는 업데이트 **전** 알림에서만 보였다(`updatemodal`). 사용자가
//! "나중에"를 누르거나, 다른 PC에서 설치본으로 깔았거나, 자동 업데이트가 조용히 끝났다면
//! 무엇이 달라졌는지 볼 방법이 없었다.
//!
//! 판정은 간단하다 — **마지막으로 실행한 판**을 설정에 적어 두고, 지금 판과 다르면 새로
//! 올라왔다는 뜻이다. 처음 설치(적힌 판이 없음)에는 보여 주지 않는다. 온보딩이 이미 있고,
//! 처음 쓰는 사람에게 변경 이력은 아무 뜻이 없다.

/// 지금 판이 지난번과 달라 "새로워진 점"을 보여야 하는가.
///
/// `seen`은 설정에 적힌 마지막 실행 판(처음이면 빈 문자열).
pub(crate) fn should_show(seen: &str, current: &str) -> bool {
    !seen.is_empty() && seen != current
}

/// 릴리스 노트에서 화면에 넣을 부분만 고른다.
///
/// 노트 끝에는 자동 업데이트용 SHA-256 줄이 붙는다(`--- / SHA256 (...) = ...`). 사용자에게는
/// 아무 뜻이 없으므로 잘라 낸다. 너무 길면 뒤를 줄인다 — 전문은 저장소에서 본다.
pub(crate) fn trim_notes(notes: &str, max_lines: usize) -> String {
    let body: Vec<&str> = notes
        .lines()
        .take_while(|l| !l.trim_start().starts_with("SHA256 ("))
        .collect();
    // 해시 앞의 구분선(---)도 함께 버린다.
    let mut body: Vec<&str> = body;
    while body.last().is_some_and(|l| l.trim().is_empty() || l.trim() == "---") {
        body.pop();
    }
    if body.len() <= max_lines {
        return body.join("\n");
    }
    let mut out = body[..max_lines].join("\n");
    out.push_str("\n\u{2026}");
    out
}

/// 새 판을 알게 됐을 때 그 노트를 적어 둔다(설치 후 다음 실행에 보여 주려고).
///
/// **버전을 함께 적는다.** 사용자가 알림을 넘기고 한참 뒤 다른 판을 직접 설치할 수도
/// 있는데, 그때 옛 노트를 보여 주면 거짓말이 된다.
pub(crate) fn stash(dir: &std::path::Path, version: &str, notes: &str) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(path(dir), format!("{version}\n{notes}"));
}

/// 적어 둔 노트를 꺼낸다 — **지금 판의 것일 때만**. 꺼내면 파일은 지운다.
pub(crate) fn take(dir: &std::path::Path, current: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path(dir)).ok()?;
    let (ver, body) = raw.split_once('\n')?;
    let _ = std::fs::remove_file(path(dir));
    (ver.trim() == current).then(|| body.to_string())
}

fn path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("whatsnew.txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_shows_only_after_the_version_actually_changed() {
        assert!(should_show("0.1.461", "0.1.462"));
        assert!(!should_show("0.1.462", "0.1.462"), "같은 판이면 보여 주지 않는다");
    }

    /// 처음 설치에는 보여 주지 않는다 — 온보딩이 이미 있고, 변경 이력은 뜻이 없다.
    #[test]
    fn a_fresh_install_does_not_see_a_changelog() {
        assert!(!should_show("", "0.1.462"));
    }

    /// 자동 업데이트용 해시 줄은 사용자에게 보이지 않아야 한다.
    #[test]
    fn the_checksum_footer_is_cut_off() {
        let notes = "새 기능이 생겼습니다\n두 번째 줄\n\n---\nSHA256 (nabiTerm-setup.exe) = deadbeef\n";
        let got = trim_notes(notes, 50);
        assert_eq!(got, "새 기능이 생겼습니다\n두 번째 줄");
        assert!(!got.contains("SHA256"));
        assert!(!got.contains("---"));
    }

    #[test]
    fn a_very_long_note_is_shortened() {
        let notes: String = (0..200).map(|i| format!("줄 {i}\n")).collect();
        let got = trim_notes(&notes, 30);
        assert_eq!(got.lines().count(), 31, "30줄 + 말줄임 한 줄");
        assert!(got.ends_with('\u{2026}'));
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("nabi-whatsnew-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn stashed_notes_come_back_for_the_matching_version() {
        let d = tmp("match");
        stash(&d, "0.1.463", "바뀐 것들");
        assert_eq!(take(&d, "0.1.463").as_deref(), Some("바뀐 것들"));
        assert!(take(&d, "0.1.463").is_none(), "한 번 꺼내면 사라진다");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 알림을 넘기고 한참 뒤 **다른 판**을 직접 설치했다면 옛 노트를 보여 주면 안 된다.
    #[test]
    fn notes_for_a_different_version_are_discarded() {
        let d = tmp("skew");
        stash(&d, "0.1.463", "463 이야기");
        assert!(take(&d, "0.1.470").is_none(), "판이 다르면 보여 주지 않는다");
        assert!(take(&d, "0.1.463").is_none(), "그리고 지워졌어야 한다");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nothing_stashed_is_not_an_error() {
        assert!(take(&tmp("none"), "0.1.1").is_none());
    }

    #[test]
    fn notes_without_a_footer_survive_intact() {
        assert_eq!(trim_notes("한 줄뿐", 10), "한 줄뿐");
        assert_eq!(trim_notes("", 10), "");
    }
}
