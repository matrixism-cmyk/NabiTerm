//! 우리 형식의 세션 파일 **가져오기** — 파일을 골라서, 형식을 가려서, 실패하면 말한다.
//!
//! ## 무엇이 잘못돼 있었나
//!
//! 세 가지가 한꺼번에 있었다(2026-08-30 전수 점검).
//!
//! 1. **고정 경로만 읽었다.** 설정 폴더의 `sessions_export.json` 하나뿐이라, 남이 보내 준
//!    파일이나 USB 로 옮겨 온 파일은 가져올 방법이 없었다. 폐쇄망에서 설정을 옮기는 것은
//!    흔한 일인데 그 길이 막혀 있었다.
//! 2. **실패를 통째로 삼켰다.** `if let Ok(..)` 라서 파일이 없어도, 형식이 깨져도 아무 일도
//!    일어나지 않고 아무 말도 없었다. 사용자는 눌렀는데 반응이 없는 것만 본다.
//! 3. **TOML 을 읽는 길이 있는데 아무도 안 썼다**(`from_toml`). JSON 만 받았다.
//!
//! ## 형식은 내용으로 가린다
//!
//! 확장자로 가리지 않는다. 사람이 이름을 바꿔 두는 일이 흔하고, 그때 확장자만 믿으면
//! 읽을 수 있는 파일을 못 읽는다. JSON 으로 먼저 시도하고 안 되면 TOML 로 읽는다.
//!
//! 실제로 목록에 담는 일은 **다른 프로그램에서 가져오는 길과 같은 함수**를 쓴다
//! (`menuactio::import_sessions`). 담는 방법이 두 벌이면 한쪽만 고쳐진다 — 중복 제거,
//! 사이드바 켜기 같은 것이 우리 형식에서만 빠지게 된다.

use nabi_i18n::tr;

impl crate::app::NabiApp {
    /// 파일을 골라 우리 형식의 세션 목록을 가져온다.
    pub(crate) fn import_session_file(&mut self) {
        let mut dlg = rfd::FileDialog::new().add_filter("sessions", &["json", "toml"]);
        if let Some(dir) = self.config_path.parent() {
            dlg = dlg.set_directory(dir);
        }
        let Some(path) = dlg.pick_file() else { return };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.notify =
                    Some((format!("\u{2715} {}: {e}", path.display()), std::time::Instant::now()));
                return;
            }
        };
        let msg = match parse_sessions(&text) {
            // 읽히긴 했는데 비어 있으면 "+0" 대신 그렇다고 말한다 — "+0" 은 실패로 보인다.
            Ok(tree) if tree.sessions.is_empty() => empty_notice(self.lang),
            // 담는 일은 공용 함수에 맡긴다 — 중복 제거·사이드바 켜기가 여기서도 그대로 돈다.
            Ok(tree) => {
                self.import_sessions(tree.sessions, "menu.importsessions", "");
                return;
            }
            Err(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

/// JSON 이면 JSON 으로, 아니면 TOML 로 읽는다. 둘 다 아니면 **JSON 쪽 이유**를 말한다.
///
/// TOML 오류를 말하면 대개 헷갈린다 — 내보내기는 JSON 으로 하므로 가져오는 것도 대개
/// JSON 이고, 그 파일이 깨졌을 때 "TOML 이 아니다"라는 말은 도움이 안 된다.
pub(crate) fn parse_sessions(text: &str) -> Result<nabi_session::SessionTree, String> {
    let first = nabi_session::export::from_json(text);
    if first.is_ok() {
        return first;
    }
    nabi_session::export::from_toml(text).or(first)
}

/// 빈 파일을 가져왔을 때 할 말 — 담을 것이 없으면 그렇다고 해야 한다.
pub(crate) fn empty_notice(lang: nabi_i18n::Lang) -> String {
    tr(lang, "sessions.importempty").to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_sessions;

    /// 내보낸 것을 그대로 다시 읽을 수 있어야 한다 — 왕복이 안 되면 내보내기가 무의미하다.
    #[test]
    fn json_과_toml_을_모두_읽는다() {
        let tree = nabi_session::SessionTree::default();
        let j = nabi_session::export::to_json(&tree).expect("json");
        assert!(parse_sessions(&j).is_ok(), "JSON 을 읽어야 한다");
        let t = nabi_session::export::to_toml(&tree).expect("toml");
        assert!(parse_sessions(&t).is_ok(), "TOML 도 읽어야 한다");
    }

    /// 아무것도 아닌 글은 **조용히 넘어가지 않고** 오류가 된다.
    #[test]
    fn 못_읽는_글은_오류다() {
        assert!(parse_sessions("이건 세션 파일이 아니다").is_err());
    }
}
