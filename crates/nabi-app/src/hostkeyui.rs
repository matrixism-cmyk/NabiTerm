//! SSH 호스트키 신뢰 모달 — 처음 연결하는 서버의 호스트·알고리즘·지문을 보여주고
//! 사용자가 신뢰/취소를 결정한다(TOFU). 결정은 Command::HostKeyDecision으로 회신.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn show_hostkey_prompt(&mut self, ctx: &egui::Context) {
        let Some((id, host, port, algo, fp)) = self.hostkey_prompt.clone() else {
            return;
        };
        let lang = self.lang;
        let (mut trust, mut cancel) = (false, false);
        // 보안 결정 모달 — 분리 창에 가려져 미확인 호스트키를 놓치지 않도록 Foreground로 띄운다.
        crate::modal::foreground_modal(ctx, "hostkey_prompt", |ui| {
            ui.heading(tr(lang, "hostkey.title"));
            ui.label(tr(lang, "hostkey.msg"));
            ui.add_space(8.0);
            egui::Grid::new("hostkey_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                ui.strong(tr(lang, "hostkey.host"));
                ui.monospace(format!("{host}:{port}"));
                ui.end_row();
                ui.strong(tr(lang, "hostkey.algo"));
                ui.monospace(&algo);
                ui.end_row();
                ui.strong(tr(lang, "hostkey.fingerprint"));
                ui.monospace(&fp);
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let btn = egui::Button::new(egui::RichText::new(tr(lang, "hostkey.trust")).color(egui::Color32::WHITE)).fill(crate::theme_ui::OK);
                if ui.add(btn).clicked() { trust = true; }
                if ui.button(tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { cancel = true; }
        });
        if trust || cancel {
            self.orch
                .send(nabi_proto::Command::HostKeyDecision { id, accept: trust });
            self.hostkey_prompt = None;
        }
    }
}
