//! 여러 탭이 열려 있을 때 창 닫기 전에 확인(실수로 닫기 방지).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 창 크기 저장 + 워크스페이스(레이아웃·스크롤백) 저장 후 프로세스 종료.
    pub(crate) fn quit(&self) -> ! {
        if self.last_win.0 >= 400.0 {
            let mut cfg = self.config.clone();
            cfg.appearance.window_w = self.last_win.0;
            cfg.appearance.window_h = self.last_win.1;
            let _ = nabi_config::save(&self.config_path, &cfg);
        }
        // 워크스페이스는 항상 저장(복원 여부는 시작 시 설정이 결정).
        self.save_workspace();
        std::process::exit(0);
    }

    pub(crate) fn handle_close(&mut self, ctx: &egui::Context) {
        // 닫기 요청: 탭이 여럿이면 확인 대화상자, 아니면 자동 저장 후 종료.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.config.terminal.confirm_close && self.dock.iter_all_tabs().count() > 1 {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_close = true;
            } else {
                self.quit();
            }
        }
        if !self.confirm_close {
            return;
        }

        let lang = self.lang;
        let n = self.dock.iter_all_tabs().count();
        let mut close = false;
        let mut cancel = false;
        // 종료 확인도 분리 창 위로 — 공통 Foreground 모달(z-order 일관화).
        crate::modal::foreground_modal(ctx, "close_confirm", |ui| {
            ui.heading(tr(lang, "close.title"));
            ui.label(format!("{} ({})", tr(lang, "close.msg"), n));
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "close.confirm")).clicked() { close = true; }
                if ui.button(tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
            ui.input(|i| {
                if i.key_pressed(egui::Key::Enter) { close = true; }
                if i.key_pressed(egui::Key::Escape) { cancel = true; }
            });
        });
        if close {
            self.quit();
        }
        if cancel {
            self.confirm_close = false;
        }
    }
}
