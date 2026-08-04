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
            for src in crate::winclip::paste_paths() {
                crate::browserops::copy_into(&src, path);
            }
        }
        // OS 드래그-아웃 — DoDragDrop은 드롭/취소까지 블로킹.
        if let Some(name) = &a.os_drag {
            crate::windnd::drag_out(&self.browser_bulk_paths(name, path));
        }
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
