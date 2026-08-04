//! 세션 메모 편집 모달 — 사이드바 세션 우클릭 ▸ 메모 편집(MenuAction::EditNote)으로 진입.
//! 메모는 config.appearance.session_notes(이름→메모)에 영속, 사이드바 호버 툴팁에 표시(MobaXterm 노트).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// note_edit가 설정돼 있으면 메모 편집 창을 그린다. 저장 시 비어 있으면 항목 제거.
    pub(crate) fn render_note_dialog(&mut self, ctx: &egui::Context) {
        let Some((name, _)) = self.note_edit.clone() else { return };
        let (lang, mut open, mut save) = (self.lang, true, false);
        egui::Window::new(format!("\u{1f4dd} {} \u{2014} {name}", tr(lang, "sessions.editnote")))
            .open(&mut open).collapsible(false).resizable(true).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if let Some((_, buf)) = self.note_edit.as_mut() {
                    ui.add(egui::TextEdit::multiline(buf).desired_width(360.0).desired_rows(5).hint_text(tr(lang, "sessions.notehint")));
                }
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "qc.save")).clicked() { save = true; }
                    if ui.button(tr(lang, "qc.cancel")).clicked() { self.note_edit = None; }
                });
            });
        if save {
            if let Some((n, buf)) = self.note_edit.take() {
                if buf.trim().is_empty() { self.config.appearance.session_notes.remove(&n); } else { self.config.appearance.session_notes.insert(n, buf); }
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
        }
        if !open {
            self.note_edit = None;
        }
    }
}
