//! SSH 호스트키 확인 모달 — 처음 보는 서버(TOFU)와 **키가 바뀐 서버**를 함께 다룬다.
//!
//! 두 경우는 무게가 전혀 다르다.
//!
//! * **처음 보는 서버**: 확인하고 넘어가는 일상. 지문을 다른 경로로 받은 값과 대조한다.
//! * **키가 바뀐 서버**: 서버를 새로 세웠거나, **누군가 중간에 끼어들었다.** 겉으로는
//!   똑같이 생겼고 우리는 구별할 수 없다.
//!
//! 그래서 화면을 다르게 만든다 — 제목·색·문구가 바뀌고, 옛 지문과 새 지문을 나란히 보여
//! 주며, 받아들이는 단추는 **한 번 더 확인**해야 눌린다. 같은 무게로 물으면 확인창이
//! 습관이 되고, 그러면 정작 위험한 순간에도 그냥 누른다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 확인을 기다리는 호스트키 한 건.
#[derive(Clone)]
pub(crate) struct HostKeyAsk {
    pub id: u64,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    /// 비어 있지 않으면 **바뀐 키**다(전에 알던 지문).
    pub old_fingerprint: String,
}

impl HostKeyAsk {
    /// 알던 키가 바뀐 경우인가.
    pub(crate) fn changed(&self) -> bool {
        !self.old_fingerprint.is_empty()
    }
}

impl NabiApp {
    pub(crate) fn show_hostkey_prompt(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.hostkey_prompt.clone() else {
            return;
        };
        let lang = self.lang;
        let changed = ask.changed();
        let (mut trust, mut cancel) = (false, false);
        // 보안 결정 모달 — 분리 창에 가려져 미확인 호스트키를 놓치지 않도록 Foreground로 띄운다.
        crate::modal::foreground_modal(ctx, "hostkey_prompt", |ui| {
            match changed {
                true => {
                    ui.heading(egui::RichText::new(tr(lang, "hostkey.changed.title")).color(crate::theme_ui::ERR));
                    ui.label(tr(lang, "hostkey.changed.msg"));
                }
                false => {
                    ui.heading(tr(lang, "hostkey.title"));
                    ui.label(tr(lang, "hostkey.msg"));
                }
            }
            ui.add_space(8.0);
            // 지문은 다른 경로로 받은 값과 대조하느라 복사할 일이 있다 — 이 표에서만 텍스트 선택 허용
            // (전역으로는 꺼 둔다: 드래그 한 번에 창 전체가 파랗게 블럭 선택되는 문제 때문).
            ui.style_mut().interaction.selectable_labels = true;
            egui::Grid::new("hostkey_grid").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                ui.strong(tr(lang, "hostkey.host"));
                ui.monospace(format!("{}:{}", ask.host, ask.port));
                ui.end_row();
                ui.strong(tr(lang, "hostkey.algo"));
                ui.monospace(&ask.algorithm);
                ui.end_row();
                if changed {
                    // 옛 지문을 먼저 둔다 — 읽는 순서가 "알던 것 → 지금 온 것"이어야 한다.
                    ui.strong(tr(lang, "hostkey.oldfp"));
                    ui.monospace(&ask.old_fingerprint);
                    ui.end_row();
                    ui.strong(tr(lang, "hostkey.newfp"));
                    ui.monospace(egui::RichText::new(&ask.fingerprint).color(crate::theme_ui::ERR));
                    ui.end_row();
                } else {
                    ui.strong(tr(lang, "hostkey.fingerprint"));
                    ui.monospace(&ask.fingerprint);
                    ui.end_row();
                }
            });
            if changed {
                ui.add_space(6.0);
                ui.colored_label(crate::theme_ui::ERR, tr(lang, "hostkey.changed.warn"));
            }
            ui.add_space(8.0);
            trust = Self::hostkey_buttons(ui, lang, changed, &mut cancel);
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
        if trust || cancel {
            self.orch.send(nabi_proto::Command::HostKeyDecision { id: ask.id, accept: trust });
            self.hostkey_prompt = None;
        }
    }

    /// 단추 줄. 바뀐 키는 **체크를 켜야** 받아들이는 단추가 살아난다.
    ///
    /// 체크 한 칸이 대단한 장벽은 아니지만, 그 한 칸을 읽는 동안 무엇을 하는지 보게 된다.
    /// 처음 보는 서버에는 붙이지 않는다 — 일상적인 확인까지 무겁게 하면 둘 다 습관이 된다.
    fn hostkey_buttons(ui: &mut egui::Ui, lang: nabi_i18n::Lang, changed: bool, cancel: &mut bool) -> bool {
        let mut trust = false;
        let sure_id = egui::Id::new("hostkey_sure");
        let mut sure: bool = ui.ctx().data(|d| d.get_temp(sure_id)).unwrap_or(false);
        if changed && ui.checkbox(&mut sure, tr(lang, "hostkey.changed.sure")).changed() {
            ui.ctx().data_mut(|d| d.insert_temp(sure_id, sure));
        }
        ui.horizontal(|ui| {
            let label = match changed {
                true => tr(lang, "hostkey.changed.accept"),
                false => tr(lang, "hostkey.trust"),
            };
            let fill = if changed { crate::theme_ui::ERR } else { crate::theme_ui::OK };
            let btn = egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE)).fill(fill);
            if ui.add_enabled(!changed || sure, btn).clicked() {
                trust = true;
                ui.ctx().data_mut(|d| d.insert_temp(sure_id, false)); // 다음 번엔 다시 확인.
            }
            if ui.button(tr(lang, "qc.cancel")).clicked() {
                *cancel = true;
            }
        });
        trust
    }
}
