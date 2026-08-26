//! **밖에서 바뀐 파일을 덮어쓰기 전에 묻는다.**
//!
//! 편집기와 터미널에서 같은 파일을 번갈아 만지는 것이 이 프로그램의 일상이다.
//! `vim`으로 고치고 편집기에서 저장하면 편집기가 들고 있던 옛 내용이 그 위를 덮고,
//! 그 변경은 **아무 말 없이 사라진다.**
//!
//! 세 가지 길을 준다. 어느 것도 내용을 잃지 않는다.
//!
//! * **덮어쓰기** — 내가 고친 것이 맞다(밖의 변경은 버린다)
//! * **다시 읽기** — 밖의 것이 맞다(내가 고친 것은 버린다)
//! * **다른 이름으로** — 둘 다 남긴다(가장 안전한 길이라 눈에 잘 띄게 둔다)

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 저장이 충돌로 멈춘 문서가 있으면 물어본다.
    pub(crate) fn show_editor_conflict(&mut self, ctx: &egui::Context) {
        let Some(pane) = self.editor_conflict else { return };
        let Some(doc) = self.editors.get(&pane) else {
            self.editor_conflict = None;
            return;
        };
        let (lang, title, gone) = (self.lang, doc.title.clone(), !doc.path.exists());

        let (mut overwrite, mut reload, mut save_as, mut close) = (false, false, false, false);
        crate::modal::foreground_modal(ctx, "editor_conflict", |ui| {
            ui.set_min_width(440.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{26a0}").size(20.0).color(crate::theme_ui::BROADCAST));
                ui.heading(tr(lang, "editconf.title"));
            });
            ui.add_space(6.0);
            ui.strong(&title);
            ui.add_space(4.0);
            // 지워진 것과 고쳐진 것은 뜻이 다르다 — 다른 말로 적는다.
            ui.label(tr(lang, if gone { "editconf.gone" } else { "editconf.changed" }));
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // 둘 다 남기는 길을 먼저·강조해 둔다.
                let keep = egui::Button::new(
                    egui::RichText::new(tr(lang, "editconf.saveas")).color(egui::Color32::WHITE),
                )
                .fill(crate::theme_ui::ACCENT_DIM);
                if ui.add(keep).clicked() {
                    save_as = true;
                }
                ui.add_space(8.0);
                if ui.button(tr(lang, "editconf.overwrite")).clicked() {
                    overwrite = true;
                }
                if !gone && ui.button(tr(lang, "editconf.reload")).clicked() {
                    reload = true;
                }
                ui.add_space(8.0);
                if ui.button(tr(lang, "guard.cancel")).clicked() {
                    close = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });

        if close {
            self.editor_conflict = None;
        }
        if save_as {
            self.editor_conflict = None;
            self.save_editor_as(pane);
        }
        if overwrite {
            self.editor_conflict = None;
            // 기준점을 지금 것으로 맞춰 두면 다음 저장이 그대로 지나간다 — 사용자가
            // "내 것이 맞다"고 답했으므로 한 번 더 묻지 않는다. 기준점은 감시자와
            // 같은 `editor_mtimes` 한 곳이다.
            if let Some(path) = self.editors.get(&pane).map(|d| d.path.clone()) {
                self.record_editor_mtime(pane, &path);
            }
            self.save_editor_doc(pane);
        }
        if reload {
            self.editor_conflict = None;
            self.reload_editor_doc(pane);
        }
    }
}
