//! 창 정렬: 메인 창 + 분리된 OS 창(멀티 뷰포트)을 바둑판식/계단식으로 배치.

use crate::app::NabiApp;

/// 정렬 방식.
#[derive(Clone, Copy)]
pub enum ArrangeMode {
    Tile,
    Cascade,
}

impl NabiApp {
    pub(crate) fn apply_arrange(&self, ctx: &egui::Context, mode: ArrangeMode) {
        match mode {
            ArrangeMode::Tile => self.tile(ctx),
            ArrangeMode::Cascade => self.cascade(ctx),
        }
    }

    /// 모든 창을 화면에 바둑판식으로 채운다(메인 + 분리 창).
    fn tile(&self, ctx: &egui::Context) {
        let monitor = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1600.0, 900.0));
        let n = 1 + self.floating.len();
        let cols = (n as f32).sqrt().ceil().max(1.0) as usize;
        let rows = n.div_ceil(cols).max(1);
        let cw = monitor.x / cols as f32;
        let ch = monitor.y / rows as f32;
        let size = egui::vec2(cw, ch);
        let cell = |i: usize| egui::pos2((i % cols) as f32 * cw, (i / cols) as f32 * ch);

        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(cell(0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        for (k, pane) in self.floating.iter().enumerate() {
            let vp = egui::ViewportId::from_hash_of(("nabi-float", pane.get()));
            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::OuterPosition(cell(k + 1)));
            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::InnerSize(size));
        }
    }

    /// 모든 창을 대각선으로 겹쳐 계단식 배치한다.
    fn cascade(&self, ctx: &egui::Context) {
        let size = egui::vec2(900.0, 600.0);
        let step = 34.0;
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(0.0, 0.0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        for (k, pane) in self.floating.iter().enumerate() {
            let off = (k as f32 + 1.0) * step;
            let vp = egui::ViewportId::from_hash_of(("nabi-float", pane.get()));
            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::OuterPosition(egui::pos2(off, off)));
            ctx.send_viewport_cmd_to(vp, egui::ViewportCommand::InnerSize(size));
        }
    }
}
