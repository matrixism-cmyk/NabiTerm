//! 비교 상대 고르기 창.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 열려 있으면 그린다.
    pub(crate) fn show_compare_picker(&mut self, ctx: &egui::Context) {
        let Some(from) = self.diff_pick else { return };
        let lang = self.lang;
        let mut open = true;
        let mut chosen: Option<PaneId> = None;
        let others = self.others_to_compare(from);
        egui::Window::new(tr(lang, "diff.pick"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.label(tr(lang, "diff.pickhint"));
                ui.separator();
                egui::ScrollArea::vertical().id_salt("diff_pick").max_height(280.0).show(ui, |ui| {
                    for (p, name) in &others {
                        if ui.selectable_label(false, name).clicked() {
                            chosen = Some(*p);
                        }
                    }
                });
            });
        if let Some(to) = chosen {
            self.diff_pick = None;
            self.compare_open_docs(from, to);
            return;
        }
        if !open {
            self.diff_pick = None;
        }
    }
}
