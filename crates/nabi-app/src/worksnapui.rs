//! 워크스페이스 스냅샷 UI(T7-2) — 저장(이름 입력)·목록(전환/삭제) 모달.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn show_snapshot_modals(&mut self, ctx: &egui::Context) {
        self.snapshot_save_modal(ctx);
        self.snapshot_list_modal(ctx);
    }

    fn snapshot_save_modal(&mut self, ctx: &egui::Context) {
        if !self.snap_save_open {
            return;
        }
        let lang = self.lang;
        let (mut save, mut close) = (false, false);
        crate::modal::foreground_modal(ctx, "snap_save", |ui| {
            ui.set_min_width(320.0);
            ui.heading(tr(lang, "snap.save"));
            ui.add_space(6.0);
            let r = ui.add(egui::TextEdit::singleline(&mut self.snap_name).hint_text(tr(lang, "snap.namehint")).desired_width(f32::INFINITY));
            nabi_editor::uiutil::focus_once(&r);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "settings.save")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    save = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
        });
        if save && !self.snap_name.trim().is_empty() {
            let name = self.snap_name.clone();
            self.save_snapshot(&name);
            self.snap_save_open = false;
        }
        if close {
            self.snap_save_open = false;
        }
    }

    fn snapshot_list_modal(&mut self, ctx: &egui::Context) {
        if !self.snap_list_open {
            return;
        }
        let lang = self.lang;
        let names = self.list_snapshots();
        let (mut open_name, mut del_name, mut close) = (None::<String>, None::<String>, false);
        crate::modal::foreground_modal(ctx, "snap_list", |ui| {
            ui.set_min_width(340.0);
            ui.heading(tr(lang, "snap.list"));
            ui.add_space(6.0);
            if names.is_empty() {
                ui.weak(tr(lang, "snap.empty"));
            }
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for n in &names {
                    ui.horizontal(|ui| {
                        if ui.button(format!("\u{21c4} {n}")).on_hover_text(tr(lang, "snap.openhint")).clicked() {
                            open_name = Some(n.clone());
                        }
                        if ui.small_button("\u{2715}").on_hover_text(tr(lang, "sessions.delete")).clicked() {
                            del_name = Some(n.clone());
                        }
                    });
                }
            });
            ui.add_space(8.0);
            if ui.button(tr(lang, "qc.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });
        if let Some(n) = open_name {
            if self.open_snapshot(&n) {
                self.snap_list_open = false;
            }
        }
        if let Some(n) = del_name {
            self.delete_snapshot(&n);
        }
        if close {
            self.snap_list_open = false;
        }
    }
}
