//! 세션 목록 가져오기/내보내기 — 메뉴 액션(menuact)에서 쓰는 파일 입출력 헬퍼.
//!
//! 가져오기는 이름 충돌을 건너뛰고 몇 건이 들어왔는지 알린다. 내보내기는 저장 대화상자로 받는다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {    /// 가져온 세션들을 추가(이름 중복은 교체)하고 저장 + 개수 토스트. 외부 형식 가져오기 공용.
    pub(crate) fn import_sessions(&mut self, imported: Vec<nabi_session::SavedSession>, label_key: &str) {
        let n = imported.len();
        for s in imported {
            self.sessions.remove(&s.name);
            self.sessions.add(s);
        }
        let dup = self.sessions.dedup(); // 여러 소스에서 가져와도 같은 대상 중복은 자동 제거.
        self.save_sessions();
        let label = tr(self.lang, label_key);
        let extra = if dup > 0 { format!(" -{dup}") } else { String::new() };
        self.notify = Some((format!("{label} +{n}{extra}"), std::time::Instant::now()));
    }

    /// 세션 내보내기 공용: 저장 대화상자로 위치를 받아 외부 형식 문자열을 쓴다.
    pub(crate) fn export_sessions_to(&mut self, data: String, name: &str, ext: &str, label_key: &str) {
        let mut dlg = rfd::FileDialog::new().set_file_name(name);
        if !ext.is_empty() {
            dlg = dlg.add_filter(ext, &[ext]);
        }
        if let Some(p) = dlg.save_file() {
            let msg = match std::fs::write(&p, data) {
                Ok(()) => format!("{} \u{2713}", tr(self.lang, label_key)),
                Err(e) => format!("\u{2715} {e}"),
            };
            self.notify = Some((msg, std::time::Instant::now()));
        }
    }

}
