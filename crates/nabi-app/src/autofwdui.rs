//! 세션별 자동 터널 편집 창.
//!
//! 세션 우클릭에서 연다. 목록 편집은 설정의 다른 목록들과 같은 모양으로 두되, 여기서만
//! 쓰는 작은 창이라 `settingslists`의 비공개 헬퍼 대신 최소한으로 그린다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 이 세션의 자동 터널을 편집한다.
    pub(crate) fn open_auto_forwards(&mut self, session: String) {
        self.fwd_edit = Some(session);
    }

    /// 열려 있으면 그린다.
    pub(crate) fn show_auto_forwards(&mut self, ctx: &egui::Context) {
        let Some(name) = self.fwd_edit.clone() else { return };
        let lang = self.lang;
        let mut open = true;
        let mut list = self.config.terminal.auto_forwards.get(&name).cloned().unwrap_or_default();
        let mut changed = false;
        egui::Window::new(format!("{} — {name}", tr(lang, "fwd.auto")))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(tr(lang, "fwd.auto.hint"));
                ui.add_space(4.0);
                let mut remove = None;
                for (i, spec) in list.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::TextEdit::singleline(spec).desired_width(260.0).hint_text("5432:db.internal:5432"))
                            .changed();
                        if ui.small_button("\u{2715}").clicked() {
                            remove = Some(i);
                        }
                        // 형식이 아니면 **그 자리에서** 알린다 — 저장하고 나서 안 열리는 것보다 낫다.
                        match crate::autofwd::parse_forward(spec) {
                            Some((l, h, r)) => ui.weak(crate::autofwd::describe(l, &h, r)),
                            None if spec.trim().is_empty() => ui.weak(""),
                            None => ui.colored_label(egui::Color32::from_rgb(0xd0, 0x4a, 0x3a), tr(lang, "fwd.auto.bad")),
                        };
                    });
                }
                if let Some(i) = remove {
                    list.remove(i);
                    changed = true;
                }
                if ui.button(tr(lang, "fwd.auto.add")).clicked() {
                    list.push(String::new());
                    changed = true;
                }
            });
        if changed {
            match list.iter().all(|s| s.trim().is_empty()) {
                true => {
                    self.config.terminal.auto_forwards.remove(&name);
                }
                false => {
                    self.config.terminal.auto_forwards.insert(name.clone(), list);
                }
            }
            self.save_config();
        }
        if !open {
            self.fwd_edit = None;
        }
    }
}
