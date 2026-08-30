//! 로컬 브라우저 ↔ 윈도우 탐색기 파일 교환(클립보드 CF_HDROP + OLE 드래그-아웃).

use crate::app::NabiApp;
use crate::browser::BrowserAct;
use std::path::Path;

impl NabiApp {
    /// 복사(클립보드) / 붙여넣기(현재 폴더로) / 드래그-아웃(탐색기로 복사)을 적용한다.
    pub(crate) fn apply_clip_drag(&mut self, a: &BrowserAct, path: &Path) {
        if let Some(name) = &a.copy {
            crate::winclip::copy_paths(&self.browser_bulk_paths(name, path));
        }
        if a.paste {
            // 못 옮긴 것이 있으면 말한다. 예전에는 결과를 버려서, 잠긴 파일이 섞여 있어도
            // 화면에는 아무 말이 없었다 — 사용자는 전부 옮겨진 줄 알고 원본을 지운다.
            let failed: usize = crate::winclip::paste_paths()
                .iter()
                .map(|src| crate::browserops::copy_into(src, path))
                .sum();
            self.note_copy_failed(failed);
        }
        // OS 드래그-아웃 — DoDragDrop은 드롭/취소까지 블로킹.
        if let Some(name) = &a.os_drag {
            crate::windnd::drag_out(&self.browser_bulk_paths(name, path));
        }
    }

    /// 복사하다 못 옮긴 것이 있으면 개수를 알린다. 0 이면 아무 말도 하지 않는다.
    ///
    /// 세 자리(붙여넣기·탭에 떨구기·사이드바에 떨구기)가 같은 말을 하도록 한곳에 둔다.
    /// 자리마다 따로 적으면 언젠가 한 자리만 고쳐진다.
    pub(crate) fn note_copy_failed(&mut self, failed: usize) {
        if failed == 0 {
            return;
        }
        self.notify = Some((
            format!("\u{26a0} {} {failed}", nabi_i18n::tr(self.lang, "browser.copyfailed")),
            std::time::Instant::now(),
        ));
    }

    /// 대상 항목의 전체 경로(다중 선택에 속하면 선택 전체).
    fn browser_bulk_paths(&self, name: &str, path: &Path) -> Vec<std::path::PathBuf> {
        let m = &self.browser.multi;
        let names: Vec<String> = if m.len() > 1 && m.contains(name) {
            m.iter().cloned().collect()
        } else {
            vec![name.to_string()]
        };
        names.iter().map(|n| path.join(n)).collect()
    }
}
