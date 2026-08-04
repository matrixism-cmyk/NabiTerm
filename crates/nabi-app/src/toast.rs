//! OSC 9 데스크톱 알림 토스트(우하단, 약 5초 후 자동 소멸).

use crate::app::NabiApp;

impl NabiApp {
    pub(crate) fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((text, when)) = self.notify.clone() else {
            return;
        };
        if when.elapsed().as_secs() >= 5 {
            self.notify = None;
            return;
        }
        egui::Area::new(egui::Id::new("nabi_toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -40.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .stroke(egui::Stroke::new(1.0, crate::theme_ui::ACCENT))
                    .show(ui, |ui| {
                        ui.label(format!("\u{1f4e3} {text}"));
                    });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(300));
    }

    /// 리사이즈 직후 화면 중앙에 "열 × 행" 크기 배지를 약 1.2초간 표시한다.
    pub(crate) fn show_resize_badge(&mut self, ctx: &egui::Context) {
        let Some((size, when)) = self.resize_badge else {
            return;
        };
        if when.elapsed().as_millis() >= 1200 {
            self.resize_badge = None;
            return;
        }
        egui::Area::new(egui::Id::new("nabi_resize_badge"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.heading(format!("{} \u{00d7} {}", size.cols(), size.rows()));
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}
