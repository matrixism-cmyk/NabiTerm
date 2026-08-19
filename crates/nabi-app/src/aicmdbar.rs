//! AI 명령 바 — AI CLI가 실행 중인 pane 상단에 그 CLI의 명령을 클릭형으로 노출.
//!
//! "/를 눌러야 나오는 명령을 처음부터 위에"(2026-08-18) → "요약명으로 보여 달라"(08-19).
//! 버튼에는 한눈에 읽히는 이름(대화 요약·새 대화…), 툴팁에는 `"/compact 설명"` 형식으로
//! 실제 명령과 설명을 함께 보여준다. 명령 표는 aicmdcmds.rs, 모드 감지는 aimode.rs.

use crate::aicmdcmds::{bar_kind, primary_commands, secondary_commands};
use nabi_i18n::tr;

/// 바에서 고른 동작 — 슬래시 명령 주입 또는 모드 순환(Shift+Tab).
pub(crate) enum BarAction {
    Cmd(String),
    ShiftTab,
}

/// pane별로 사용자가 바에서 고른 값(모델·노력) — 버튼에 현재 상태를 보여주려고 기억한다.
/// CLI가 상태줄로 알려주는 값(pane_status "model")이 있으면 그쪽이 우선(실시간·권위).
#[derive(Default, Clone)]
pub(crate) struct AiPicks {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// 바 버튼 글자 크기 — 드롭다운(menu_button)과 일반 버튼의 **높이를 같게** 맞춘다.
/// small_button은 여백 규칙이 달라 한 줄에서 높이가 어긋난다(사용자 지적 2026-08-19).
const BAR_TEXT: f32 = 12.0;

fn bar_text(s: impl Into<String>) -> egui::RichText {
    egui::RichText::new(s.into()).size(BAR_TEXT)
}

impl crate::tabs::TermTabViewer<'_> {
    /// pane에 AI 명령 바를 그린다(설정 꺼짐/비AI pane이면 아무것도 안 그림).
    /// 클릭된 명령은 "/cmd\r"로, 모드 버튼은 CSI Z(shift+tab)로 그 pane에 보낸다.
    pub(crate) fn ai_bar(&mut self, ui: &mut egui::Ui, pane: nabi_types::PaneId) {
        if !self.ai_cmd_bar {
            return;
        }
        let Some(kind) = self.run_cmd.get(&pane).and_then(|c| bar_kind(c)) else { return };
        // 모델은 CLI 상태줄(pane_status)이 있으면 그 값, 없으면 사용자가 바에서 고른 값.
        let picks = self.ai_picks.get(&pane).cloned().unwrap_or_default();
        let model = self
            .pane_status
            .get(&pane)
            .and_then(|m| m.get("model"))
            .cloned()
            .or(picks.model);
        let action = show_bar(ui, self.lang, kind, self.pane_ai_mode(pane), model.as_deref(), picks.effort.as_deref());
        let Some(action) = action else { return };
        let data = match action {
            BarAction::Cmd(cmd) => {
                // 사용자가 고른 값을 기억해 다음 프레임부터 버튼에 현재 상태로 보여준다.
                if let Some(v) = cmd.strip_prefix("/model ") {
                    self.ai_picks.entry(pane).or_default().model = Some(v.to_string());
                } else if let Some(v) = cmd.strip_prefix("/effort ") {
                    self.ai_picks.entry(pane).or_default().effort = Some(v.to_string());
                }
                let mut d = cmd.into_bytes();
                d.push(b'\r');
                d
            }
            BarAction::ShiftTab => crate::aimode::SHIFT_TAB.to_vec(),
        };
        self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
    }

    /// pane 화면 하단에서 현재 승인/권한 모드를 읽는다(CLI가 상태 줄에 쓰는 문구 — aimode.rs).
    fn pane_ai_mode(&self, pane: nabi_types::PaneId) -> &'static str {
        self.orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).map(|v| v.model.clone()))
            .and_then(|md| md.lock().ok().map(|m| crate::aimode::detect_mode(&m.visible_bottom_text(4))))
            .unwrap_or("aimode.unknown")
    }
}

/// 명령 바 UI. 반환 = 고른 동작. `model`/`effort`가 있으면 그 버튼은 명령 이름 대신
/// **현재 값**을 보여준다(예: `opus`, `high`).
pub(crate) fn show_bar(
    ui: &mut egui::Ui,
    lang: nabi_i18n::Lang,
    kind: &'static str,
    mode: &'static str,
    model: Option<&str>,
    effort: Option<&str>,
) -> Option<BarAction> {
    let mut send = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // 모든 버튼이 같은 여백을 쓰게 고정 — 드롭다운/일반 버튼 높이 일치.
        ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
        ui.label(bar_text(format!("\u{1f916} {kind}")).color(crate::theme_ui::ACCENT));
        // 모드 순환 버튼: 누를 때마다 shift+tab을 보내고, 현재 모드를 화면에서 읽어 보여준다.
        if ui
            .button(bar_text(format!("\u{21e7}\u{21e5} {}", tr(lang, mode))))
            .on_hover_text(tr(lang, "aicb.shifttab"))
            .clicked()
        {
            send = Some(BarAction::ShiftTab);
        }
        ui.separator();
        for bc in primary_commands(kind) {
            // /model·/effort는 명령 이름 대신 현재 값을 노출(사용자 요청) — 없으면 요약명.
            let cur = match bc.cmd {
                "/model" => model,
                "/effort" => effort,
                _ => None,
            };
            let label = cur.map_or_else(|| tr(lang, bc.label).to_string(), str::to_string);
            let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
            if bc.sub.is_empty() {
                if ui.button(bar_text(label)).on_hover_text(tip).clicked() {
                    send = Some(BarAction::Cmd(bc.cmd.to_string()));
                }
            } else {
                ui.menu_button(bar_text(format!("{label} \u{25be}")), |ui| {
                    for (label, cmd) in bc.sub {
                        if ui.button(*label).clicked() {
                            send = Some(BarAction::Cmd((*cmd).to_string()));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tip);
            }
        }
        ui.menu_button(bar_text("\u{22ef}"), |ui| {
            for bc in secondary_commands(kind) {
                let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
                if ui.button(tr(lang, bc.label)).on_hover_text(tip).clicked() {
                    send = Some(BarAction::Cmd(bc.cmd.to_string()));
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(tr(lang, "aicb.more"));
    });
    send
}
