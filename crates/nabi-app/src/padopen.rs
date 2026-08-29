//! 탐색기 우클릭 **"nabiPad로 편집"** — 이미 떠 있으면 그쪽에, 아니면 편집기만 띄운다.
//!
//! ## 왜 이게 있어야 하나
//!
//! nabiPad 는 우리 세 기둥 중 하나인데, 지금까지는 **터미널을 먼저 띄워야** 쓸 수 있었다.
//! 메모장이나 VS Code 처럼 파일을 오른쪽 눌러 바로 열 수 있어야 편집기라고 할 수 있다
//! (사용자 요청 2026-08-29).
//!
//! ## 어떻게 도는가
//!
//! 설치 프로그램이 등록하는 명령은 `nabi.exe --edit "%1"` 이다.
//!
//! 1. 나비텀이 이미 떠 있으면 → 파이프로 넘기고 **조용히 끝난다**(창이 둘 뜨지 않게)
//! 2. 없으면 → **편집기만** 띄운다. 터미널 창은 만들지 않는다.
//!
//! 2번이 `--open-here` 와 다른 점이다. 그쪽은 터미널을 여는 것이 목적이지만 이쪽은
//! 편집기가 목적이라, 터미널까지 뜨면 사용자가 부른 적 없는 창이 생긴다.

/// `--edit` 처리 결과.
pub(crate) enum Outcome {
    /// 떠 있는 인스턴스에 넘겼다 — 이 프로세스는 끝내면 된다.
    Delegated,
    /// 넘길 곳이 없다 — 이 파일을 편집기로 연다(터미널 없이).
    PadOnly(String),
}

/// 인자에서 `--edit <경로>` 를 찾아 처리한다. 그 인자가 없으면 None.
pub(crate) fn handle(args: &[String]) -> Option<Outcome> {
    let i = args.iter().position(|a| a == "--edit")?;
    let path = clean(args.get(i + 1).map(String::as_str).unwrap_or_default());
    if path.is_empty() {
        // 경로 없이 불렀다 — 빈 편집기를 띄운다. 아무 일도 안 하는 것보다 낫다.
        return Some(Outcome::PadOnly(String::new()));
    }
    if crate::handoff::delegate("open-file", "path", &path) {
        return Some(Outcome::Delegated);
    }
    // 떠 있지만 말이 안 통하면(막 죽는 중 등) 새로 띄운다 — 아무 일도 일어나지 않는 것보다
    // 창이 하나 더 뜨는 쪽이 덜 나쁘다.
    Some(Outcome::PadOnly(path))
}

/// 탐색기가 주는 경로를 다듬는다.
///
/// `--open-here` 와 달리 **파일을 폴더로 바꾸지 않는다.** 여기서는 그 파일 자체가 목적이다.
pub(crate) fn clean(raw: &str) -> String {
    raw.trim().trim_matches('"').to_string()
}

impl crate::app::NabiApp {
    /// 탐색기에서 `--edit` 로 부른 파일을 **한 번만** 연다.
    ///
    /// GUI 가 뜨기 전에 정해지므로 환경 변수로 넘어온다(`NABI_START_CWD` 와 같은 방식).
    /// 읽고 나서 지운다 — 안 지우면 창을 새로 열 때마다 같은 파일이 또 열린다.
    pub(crate) fn open_startup_file(&mut self) {
        let Ok(v) = std::env::var("NABI_OPEN_FILE") else { return };
        std::env::remove_var("NABI_OPEN_FILE");
        if v.is_empty() {
            self.open_empty_pad();
            return;
        }
        self.open_editor_local(std::path::PathBuf::from(v));
    }
}

#[cfg(test)]
mod tests {
    use super::{clean, handle, Outcome};

    #[test]
    fn it_only_acts_on_its_own_flag() {
        assert!(handle(&["nabi.exe".into()]).is_none());
        assert!(handle(&["nabi.exe".into(), "--open-here".into(), "C:\\".into()]).is_none());
    }

    #[test]
    fn quotes_and_spaces_are_trimmed() {
        assert_eq!(clean(r#"  "C:\Program Files\a.txt"  "#), r"C:\Program Files\a.txt");
    }

    #[test]
    fn a_file_stays_a_file() {
        // 여기서 폴더로 바꾸면 사용자가 고른 파일이 열리지 않는다.
        // (--open-here 는 반대로 폴더로 바꾼다 — 목적이 다르다.)
        assert_eq!(clean(r"C:\일감\메모.txt"), r"C:\일감\메모.txt");
    }

    #[test]
    fn no_path_still_opens_an_editor() {
        // 빈 편집기라도 뜨는 편이 아무 일도 안 일어나는 것보다 낫다.
        let r = handle(&["nabi.exe".into(), "--edit".into()]);
        assert!(matches!(r, Some(Outcome::PadOnly(p)) if p.is_empty()));
    }
}
