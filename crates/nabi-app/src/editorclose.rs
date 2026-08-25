//! nabiPad 문서 닫기 — 미저장 확인 모달과 실제 탭/창 정리. 열기는 editoropen.rs.
//!
//! 확인 모달은 분리 창 위에도 떠야 하므로 `modal::foreground_modal`을 쓴다
//! (egui::Window는 분리 창 아래로 가려진다).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn request_editor_close(&mut self, p: nabi_types::PaneId) {
        if self.editors.get(&p).map(|e| e.dirty).unwrap_or(false) {
            self.editor_close_ask = Some(p);
        } else {
            self.close_editor_pane(p);
        }
    }

    /// 에디터 pane을 완전히 닫는다(문서·도크 탭·분리 창 정리).
    pub(crate) fn close_editor_pane(&mut self, p: nabi_types::PaneId) {
        crate::padrecover::drop_one(&self.cfg_dir(), p.0); // 닫았다 = 사용자가 버린 것.
        // 실수로 닫았을 수 있다 — 경로만 기억해 둔다(내용은 padrecover 소관).
        if let Some(d) = self.editors.get(&p) {
            let path = d.path.to_string_lossy().into_owned();
            crate::reopenclosed::remember(&mut self.closed_docs, &path);
        }
        self.editors.remove(&p);
        if let Some(loc) = self.dock.find_tab(&p) {
            self.dock.remove_tab(loc);
        }
        self.floating.retain(|x| *x != p);
        self.floating_shown.remove(&p);
    }

    /// 미저장 변경 닫기 확인 모달 — 메인 창의 도킹 에디터만 여기서 그린다.
    /// 분리 창(viewport) 에디터의 모달은 그 창의 ctx에서 render_editor_close_confirm로 그린다
    /// (메인 ctx에 그리면 nabiPad 별도 OS 창 뒤로 가려져 선택 불가 — 사용자 보고).
    pub(crate) fn editor_close_modal(&mut self, ctx: &egui::Context) {
        let Some(p) = self.editor_close_ask else { return };
        if self.floating.contains(&p) {
            return; // 분리 창 에디터는 해당 viewport에서 그린다.
        }
        self.render_editor_close_confirm(ctx, p);
    }

    /// 닫기 확인 모달 본문(메인·분리 창 공용). 주어진 ctx(메인 또는 viewport) 위에 Foreground로 띄운다.
    pub(crate) fn render_editor_close_confirm(&mut self, ctx: &egui::Context, p: nabi_types::PaneId) {
        let lang = self.lang;
        let name = self.editors.get(&p).map(|e| e.title.clone()).unwrap_or_default();
        let (mut save_close, mut discard, mut cancel) = (false, false, false);
        crate::modal::foreground_modal(ctx, "editor_close_modal", |ui| {
            ui.heading(tr(lang, "nabipad.closeask"));
            ui.label(format!("{name} — {}", tr(lang, "nabipad.unsaved")));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "nabipad.saveclose")).clicked() { save_close = true; }
                if ui.button(tr(lang, "nabipad.discard")).clicked() { discard = true; }
                if ui.button(tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
        });
        if save_close {
            self.save_editor_doc(p);
            self.close_editor_pane(p);
            self.editor_close_ask = None;
        } else if discard {
            self.close_editor_pane(p);
            self.editor_close_ask = None;
        } else if cancel {
            self.editor_close_ask = None;
        }
    }

}
