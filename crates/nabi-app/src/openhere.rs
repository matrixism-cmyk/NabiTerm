//! 탐색기 우클릭 **"nabiTerm에서 열기"** — 이미 떠 있으면 새 pane, 아니면 새로 띄운다.
//!
//! Windows Terminal이 같은 기능을 제공하고, 폴더에서 바로 터미널을 여는 것은 이 부류
//! 프로그램의 기본 동작에 가깝다(사용자 요청 2026-08-25).
//!
//! ## 어떻게 판단하는가
//!
//! 설치 프로그램이 등록한 명령은 `nabi.exe --open-here "%V"`다. 그 프로세스는 GUI를 띄우기
//! **전에** 여기를 지난다.
//!
//! 1. 설정 폴더의 접속 정보([`nabi_control::discovery`])로 이미 떠 있는 인스턴스를 찾는다.
//! 2. 있으면 파이프로 `open-here`를 보내고 **조용히 끝난다**(창 두 개가 뜨지 않게).
//! 3. 없으면 그 폴더를 시작 디렉터리로 삼아 평소대로 GUI를 띄운다.

/// `--open-here` 처리 결과.
pub(crate) enum Outcome {
    /// 떠 있는 인스턴스에 넘겼다 — 이 프로세스는 끝내면 된다.
    Delegated,
    /// 넘길 곳이 없다 — 이 경로를 시작 디렉터리로 GUI를 띄운다.
    StartHere(String),
}

/// 인자에서 `--open-here <경로>`를 찾아 처리한다. 그 인자가 없으면 None.
pub(crate) fn handle(args: &[String]) -> Option<Outcome> {
    let i = args.iter().position(|a| a == "--open-here")?;
    let path = args.get(i + 1).cloned().unwrap_or_default();
    let path = normalize(&path);
    if crate::handoff::delegate("open-here", "path", &path) {
        return Some(Outcome::Delegated);
    }
    // 떠 있지만 말이 안 통하면(막 죽는 중 등) 새로 띄우는 편이 낫다 — 아무 일도
    // 일어나지 않는 것보다 창이 하나 더 뜨는 쪽이 사용자에게 덜 나쁘다.
    Some(Outcome::StartHere(path))
}

/// 탐색기가 주는 경로를 다듬는다.
///
/// 폴더 배경에서 부르면 끝에 역슬래시가 붙어 오고(`C:\일감\`), 드라이브 루트는 그 역슬래시가
/// 있어야 한다(`C:\`). 파일을 골랐다면 그 파일이 든 폴더를 쓴다.
pub(crate) fn normalize(raw: &str) -> String {
    let t = raw.trim().trim_matches('"');
    if t.is_empty() {
        return String::new();
    }
    let p = std::path::Path::new(t);
    if p.is_file() {
        return p.parent().map(|d| d.display().to_string()).unwrap_or_else(|| t.to_string());
    }
    // 드라이브 루트("C:\")는 역슬래시를 남기고, 그 밖에는 뗀다.
    let trimmed = t.trim_end_matches(['\\', '/']);
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        return format!("{trimmed}\\");
    }
    if trimmed.is_empty() { t.to_string() } else { trimmed.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_only_acts_on_its_own_flag() {
        assert!(handle(&["nabi.exe".into()]).is_none());
        assert!(handle(&["nabi.exe".into(), "mcp".into()]).is_none());
    }

    /// 폴더 배경에서 부르면 끝에 역슬래시가 붙어 온다 — 떼고 쓴다.
    #[test]
    fn a_trailing_separator_is_removed() {
        assert_eq!(normalize("C:\\일감\\"), "C:\\일감");
        assert_eq!(normalize("C:/work/"), "C:/work");
    }

    /// **드라이브 루트만은 역슬래시를 남긴다** — "C:"는 폴더가 아니라 "현재 폴더"를 뜻한다.
    #[test]
    fn a_drive_root_keeps_its_separator() {
        assert_eq!(normalize("C:\\"), "C:\\");
        assert_eq!(normalize("D:\\"), "D:\\");
    }

    #[test]
    fn quotes_and_spaces_are_trimmed() {
        assert_eq!(normalize("  \"C:\\Program Files\"  "), "C:\\Program Files");
    }

    #[test]
    fn an_empty_path_stays_empty() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("   "), "");
    }

    /// 파일을 고르면 그 파일이 든 폴더를 쓴다(탐색기가 파일 경로를 줄 수도 있다).
    #[test]
    fn a_file_becomes_its_folder() {
        let d = std::env::temp_dir().join(format!("nabi-oh-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("x.txt");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(normalize(&f.display().to_string()), d.display().to_string());
        let _ = std::fs::remove_dir_all(&d);
    }
}
