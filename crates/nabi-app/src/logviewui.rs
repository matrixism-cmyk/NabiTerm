//! 진단 로그 보기 창 — logview가 읽어 온 것을 화면에 놓는다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 로그 보기 창을 연다(도움말▸진단 로그·팔레트).
    pub(crate) fn open_log_view(&mut self) {
        let dir = self.cfg_dir().join("logs");
        let _ = std::fs::create_dir_all(&dir);
        self.log_view = Some(crate::logview::latest(&dir));
    }

    /// 열려 있으면 그린다. 오류만 보기·복사·폴더 열기.
    pub(crate) fn show_log_view(&mut self, ctx: &egui::Context) {
        let Some(loaded) = self.log_view.as_ref() else { return };
        let lang = self.lang;
        let mut open = true;
        let (mut copy, mut reload, mut open_dir) = (None, false, false);
        let id = egui::Id::new("logview_only_problems");
        let mut only: bool = ctx.data(|d| d.get_temp(id)).unwrap_or(false);
        egui::Window::new(tr(lang, "help.diaglogs"))
            .open(&mut open)
            .default_size([820.0, 520.0])
            .collapsible(false)
            .show(ctx, |ui| {
                let Some(log) = loaded else {
                    ui.label(tr(lang, "logview.none"));
                    return;
                };
                ui.horizontal(|ui| {
                    ui.label(&log.file);
                    if log.truncated {
                        ui.weak(tr(lang, "logview.tail"));
                    }
                });
                ui.separator();
                let shown = if only { crate::logview::only_problems(&log.body) } else { log.body.clone() };
                ui.horizontal(|ui| {
                    // 지원 요청에 붙일 것을 사용자가 고르지 않아도 되게 — 오류만 걸러 한 번에 복사.
                    ui.checkbox(&mut only, tr(lang, "logview.onlyproblems"));
                    if ui.button(tr(lang, "logview.copy")).clicked() {
                        copy = Some(shown.clone());
                    }
                    if ui.button(tr(lang, "logview.reload")).clicked() {
                        reload = true;
                    }
                    if ui.button(tr(lang, "logview.folder")).clicked() {
                        open_dir = true;
                    }
                });
                ui.add_space(4.0);
                if only && shown.is_empty() {
                    ui.colored_label(crate::theme_ui::OK, tr(lang, "logview.clean"));
                    return;
                }
                egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    // 선택·복사가 되도록 편집 가능 위젯에 읽기 전용으로 넣는다.
                    let mut text = shown;
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            });
        ctx.data_mut(|d| d.insert_temp(id, only));
        if let Some(t) = copy.filter(|t: &String| !t.is_empty()) {
            ctx.copy_text(t);
            self.notify = Some((tr(lang, "logview.copied").to_string(), std::time::Instant::now()));
        }
        if open_dir {
            let _ = std::process::Command::new("explorer").arg(self.cfg_dir().join("logs")).spawn();
        }
        if reload {
            self.open_log_view();
        } else if !open {
            self.log_view = None;
        }
    }
}
