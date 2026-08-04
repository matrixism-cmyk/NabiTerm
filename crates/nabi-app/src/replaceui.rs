//! 파일에서 찾아 바꾸기(Replace in Files) 모달 — 현재 로컬 브라우저 폴더 대상.
//! 안전: 미리보기(계산만)로 영향 범위 확인 후 "적용"에서만 파일을 기록한다(대소문자 구분).

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

impl NabiApp {
    /// 찾아 바꾸기 모달을 그린다(replace_open일 때). 적용 시 파일 기록 후 닫힘.
    pub(crate) fn show_replace_in_files(&mut self, ctx: &egui::Context) {
        if !self.replace_open {
            return;
        }
        let lang = self.lang;
        let mut close = false;
        crate::modal::foreground_modal(ctx, "replace_in_files", |ui| {
            ui.heading(tr(lang, "replace.title"));
            ui.label(format!("{}: {}", tr(lang, "replace.scope"), self.browser.path.display()));
            ui.add_space(4.0);
            egui::Grid::new("replace_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(tr(lang, "replace.find"));
                ui.add(egui::TextEdit::singleline(&mut self.replace_find).desired_width(260.0));
                ui.end_row();
                ui.label(tr(lang, "replace.to"));
                ui.add(egui::TextEdit::singleline(&mut self.replace_to).desired_width(260.0));
                ui.end_row();
            });
            if let Some((f, m)) = self.replace_count {
                ui.colored_label(crate::theme_ui::ACCENT, format!("{m} \u{00d7} · {f} files"));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let has = !self.replace_find.is_empty();
                if ui.add_enabled(has, egui::Button::new(tr(lang, "replace.preview"))).clicked() {
                    self.replace_count = Some(crate::findfiles::replace_in_dir(&self.browser.path, &self.replace_find, &self.replace_to, false, 3000));
                }
                if ui.add_enabled(has, egui::Button::new(tr(lang, "replace.apply"))).clicked() {
                    let (f, m) = crate::findfiles::replace_in_dir(&self.browser.path, &self.replace_find, &self.replace_to, true, 3000);
                    self.notify = Some((format!("{} {m}\u{00d7}/{f}", tr(lang, "replace.title")), Instant::now()));
                    close = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.replace_open = false;
            self.replace_count = None;
        }
    }
}
