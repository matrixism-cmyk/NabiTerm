//! OS 창 제목 — "nabiTerm v<버전>"(nabidrive식 버전 표시).

use crate::app::NabiApp;

impl NabiApp {
    pub(crate) fn update_window_title(&mut self, ctx: &egui::Context) {
        // 편집기만 띄운 창은 **nabiPad 라고 적는다.** 작업 표시줄에서 고를 때 터미널
        // 창과 구별되어야 하고, 사용자가 부른 이름이 그것이다.
        let name = match self.pad_only {
            true => "nabiPad",
            false => "nabiTerm",
        };
        let full = format!("{name} v{}", env!("CARGO_PKG_VERSION"));
        if full != self.window_title {
            self.window_title = full.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(full));
        }
    }
}
