//! OS 창 제목 — "nabiTerm v<버전>"(nabidrive식 버전 표시).

use crate::app::NabiApp;

impl NabiApp {
    pub(crate) fn update_window_title(&mut self, ctx: &egui::Context) {
        let full = format!("nabiTerm v{}", env!("CARGO_PKG_VERSION"));
        if full != self.window_title {
            self.window_title = full.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(full));
        }
    }
}
