//! 다른 프로그램에서 세션을 **가져오는 길들** — 읽고, 해석하고, **결과를 말한다.**
//!
//! ## 무엇이 잘못돼 있었나
//!
//! 가져오기 일곱 갈래가 전부 같은 모양이었다(2026-08-30 메뉴 전수 점검).
//!
//! ```ignore
//! if let Ok(text) = std::fs::read_to_string(&path) {
//!     … 가져온다 …
//! }
//! ```
//!
//! 파일이 없으면 **아무 일도 일어나지 않고 아무 말도 없다.** 특히 `~/.ssh/config` 는
//! 윈도우에서 없는 편이 흔한데, 그때 사용자가 보는 것은 "눌렀는데 반응이 없는 메뉴"다.
//! 프로그램이 고장 난 것과 구별되지 않는다.
//!
//! 갈래마다 고치지 않고 **한 규칙으로 모았다.** 하나씩 고치면 다음에 가져오기를 하나 더
//! 만들 때 또 같은 모양이 생긴다.
//!
//! ## 규칙
//!
//! * **취소는 실패가 아니다.** 파일 고르기를 닫았으면 아무 말도 하지 않는다.
//! * **못 읽으면 왜인지 말한다.** 경로와 운영체제가 준 이유를 그대로 붙인다.
//! * **읽었는데 비어 있으면 그렇다고 말한다.** `+0` 은 성공처럼 보여서 더 헷갈린다.
//! * 담는 일은 `menuactio::import_sessions` 하나로 — 중복 제거·사이드바 켜기가 함께 돈다.

use nabi_i18n::tr;
use nabi_session::SavedSession;
use std::path::Path;

/// 가져오기 한 번의 결과.
pub(crate) enum Got {
    /// 사용자가 파일 고르기를 닫았다 — 아무 말도 하지 않는다.
    Cancelled,
    Sessions(Vec<SavedSession>),
    Failed(String),
}

impl crate::app::NabiApp {
    /// 가져온 결과를 한 규칙으로 처리한다. 모든 가져오기가 이 자리를 지난다.
    pub(crate) fn finish_import(&mut self, got: Got, label_key: &str, folder: &str) {
        let msg = match got {
            Got::Cancelled => return,
            Got::Sessions(v) if v.is_empty() => {
                format!("{} \u{2014} {}", tr(self.lang, label_key), tr(self.lang, "import.none"))
            }
            Got::Sessions(v) => return self.import_sessions(v, label_key, folder),
            Got::Failed(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

/// 파일을 읽어 글로 만든다 — 인코딩은 자동으로 가린다(UTF-16·ANSI 흔하다).
///
/// 실패하면 **경로와 이유**를 함께 돌려준다. 경로가 없으면 어느 파일이 문제인지 알 수 없다.
pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    match std::fs::read(path) {
        Ok(b) => Ok(crate::editload::decode(&b).0),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// 자동 탐지 → 파일 고르기 → 읽기 → 해석까지 한 줄로.
///
/// 해석기가 아무것도 못 찾으면 빈 목록이 되고, 그것은 `finish_import` 가
/// "가져올 것이 없다"로 말한다 — 해석기마다 그 판단을 하지 않아도 된다.
pub(crate) fn from_file(
    auto: Option<std::path::PathBuf>,
    pick: impl FnOnce() -> Option<std::path::PathBuf>,
    parse: impl FnOnce(&str) -> Vec<SavedSession>,
) -> Got {
    let Some(p) = auto.or_else(pick) else {
        return Got::Cancelled;
    };
    match read_text(&p) {
        Ok(t) => Got::Sessions(parse(&t)),
        Err(e) => Got::Failed(e),
    }
}

/// 파일이 아니라 **글을 이미 손에 쥔** 경우(레지스트리에서 뽑은 것 등).
///
/// 자동으로 못 얻었으면 파일을 고르게 하고, 그것도 없으면 취소다.
pub(crate) fn from_text_or_file(
    auto: Option<String>,
    pick: impl FnOnce() -> Option<std::path::PathBuf>,
    parse: impl FnOnce(&str) -> Vec<SavedSession>,
) -> Got {
    if let Some(t) = auto {
        return Got::Sessions(parse(&t));
    }
    from_file(None, pick, parse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 고르기를_닫으면_취소다() {
        let got = from_file(None, || None, |_| Vec::new());
        assert!(matches!(got, Got::Cancelled), "취소는 오류가 아니다");
    }

    #[test]
    fn 없는_파일은_이유를_말한다() {
        let p = std::path::PathBuf::from("Z:/없는폴더/없는파일.ini");
        let got = from_file(Some(p.clone()), || None, |_| Vec::new());
        match got {
            Got::Failed(e) => assert!(e.contains("없는파일"), "경로가 들어 있어야 한다: {e}"),
            _ => panic!("실패로 나와야 한다"),
        }
    }

    #[test]
    fn 읽히면_해석기에_넘긴다() {
        let dir = std::env::temp_dir().join("nabi-import-test");
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("a.txt");
        std::fs::write(&f, "hello").expect("쓰기");
        let got = from_file(Some(f.clone()), || None, |t| {
            assert_eq!(t, "hello");
            vec![SavedSession {
                name: "x".into(),
                folder: None,
                kind: nabi_session::SessionKind::Local { shell: Default::default() },
                on_connect: None,
                cwd: None,
                is_ftp: false,
                open_sftp: false,
                tag: Default::default(),
            }]
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(got, Got::Sessions(v) if v.len() == 1));
    }
}
