//! 위험 명령 확인창 — `guard.rs`가 붙잡은 입력을 사람에게 보여 주고 묻는다.
//!
//! ## 무엇을 보여 주는가
//!
//! "정말 실행하시겠습니까?"만 띄우면 사람은 읽지 않고 누른다. **무엇이·어디서·왜**를
//! 함께 보여야 판단이 된다.
//!
//! * 무엇 — 화면에서 읽어 낸 명령 그대로
//! * 어디서 — 세션 이름과 표식(운영/스테이징), 브로드캐스트면 몇 개 창인지
//! * 왜 — 지우기·디스크·전원·덮어쓰기·권한 중 무엇에 걸렸는지
//!
//! 기본 단추는 **취소**다. 엔터를 한 번 더 눌러 지나가 버리는 일이 없어야 한다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 붙잡힌 입력이 있으면 확인창을 그린다.
    pub(crate) fn show_guard(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending_send.clone() else { return };
        let lang = self.lang;
        let tag = self.pane_tag(p.pane);
        let title = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p.pane).map(|v| v.title.clone()))
            .unwrap_or_default();

        let mut send = false;
        let mut cancel = false;
        crate::modal::foreground_modal(ctx, "guard_confirm", |ui| {
            ui.set_min_width(460.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{26a0}").size(22.0).color(crate::theme_ui::ERR));
                ui.heading(tr(lang, "guard.title"));
            });
            ui.add_space(6.0);

            // 어디서 — 표식이 있으면 그 색으로 함께 적는다.
            ui.horizontal_wrapped(|ui| {
                ui.label(tr(lang, "guard.where"));
                ui.strong(&title);
                if tag != nabi_session::SessionTag::None {
                    let (r, g, b) = tag.rgb();
                    ui.colored_label(egui::Color32::from_rgb(r, g, b), tr(lang, tag.key()));
                }
            });
            if p.panes.len() > 1 {
                // 여러 창에 동시에 나가는 것이 가장 위험하다 — 눈에 띄게 적는다.
                ui.colored_label(
                    crate::theme_ui::BROADCAST,
                    format!("\u{1f4e2} {} {}", tr(lang, "guard.broadcast"), p.panes.len()),
                );
            }

            ui.add_space(8.0);
            ui.label(tr(lang, "guard.what"));
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&p.command).monospace().color(crate::theme_ui::ERR))
                        .wrap(),
                );
            });

            ui.add_space(6.0);
            ui.label(format!("{} {}", tr(lang, "guard.why"), tr(lang, p.why.key())));
            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // 기본은 취소. 눌러서 지나가는 것이 아니라, 눌러서 멈추는 것이 쉬워야 한다.
                if ui.button(tr(lang, "guard.cancel")).clicked() {
                    cancel = true;
                }
                ui.add_space(8.0);
                let go = egui::Button::new(
                    egui::RichText::new(tr(lang, "guard.send")).color(egui::Color32::WHITE),
                )
                .fill(crate::theme_ui::ERR);
                if ui.add(go).clicked() {
                    send = true;
                }
                ui.add_space(12.0);
                ui.weak(tr(lang, "guard.hint"));
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

        if cancel {
            // 엔터를 삼킨다 — 셸에는 아무것도 가지 않는다. 친 글자는 화면에 그대로 남는다.
            self.pending_send = None;
        }
        if send {
            self.release_pending();
        }
    }

    /// 확인을 지난 입력을 **손대지 않은 채** 흘려보낸다.
    fn release_pending(&mut self) {
        let Some(p) = self.pending_send.take() else { return };
        let data = bytes::Bytes::from(p.data);
        // 붙잡을 때의 대상을 그대로 쓴다 — 지금 다시 계산하면 그 사이 바뀌었을 수 있다.
        if !p.panes.is_empty() {
            self.orch.send(nabi_proto::Command::Broadcast { panes: p.panes, data });
        } else {
            self.orch.send(nabi_proto::Command::WriteInput { pane: p.pane, data });
        }
    }
}
