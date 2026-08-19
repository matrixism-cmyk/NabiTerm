//! AI 명령 바 — AI CLI가 실행 중인 pane 상단에 그 CLI의 명령을 클릭형으로 노출.
//!
//! "/를 눌러야 나오는 명령을 처음부터 위에"(2026-08-18) → "요약명으로"(08-19) →
//! "열려 있으면 색으로 표시하고 다시 누르면 닫히게"(08-19). 버튼에는 한눈에 읽히는
//! 이름(대화 요약·새 대화…), 툴팁에는 `"/compact 설명"` 형식으로 실제 명령과 설명을 보여준다.
//! 명령 표는 aicmdcmds.rs, 모드 감지는 aimode.rs.

use crate::aicmdcmds::{bar_kind, primary_commands, secondary_commands};
use nabi_i18n::tr;

/// 바에서 고른 동작.
pub(crate) enum BarAction {
    /// 슬래시 명령 전송(명령, 이 명령이 CLI 화면을 여는가).
    Cmd(String, bool),
    /// 열어 둔 화면 닫기(ESC 전송).
    Esc,
    /// 권한 모드 순환(Shift+Tab 전송).
    ShiftTab,
}

/// pane별 명령 바 상태 — 버튼에 현재 값을 보여주고 열린 화면을 추적한다.
#[derive(Default, Clone)]
pub(crate) struct AiPicks {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// 바 버튼으로 지금 열어 둔 명령(활성 색 표시 + 재클릭 시 ESC).
    pub active: Option<String>,
}

/// 바 버튼 글자 크기 — 드롭다운(menu_button)과 일반 버튼의 **높이를 같게** 맞춘다.
const BAR_TEXT: f32 = 12.0;

fn bar_text(s: impl Into<String>) -> egui::RichText {
    egui::RichText::new(s.into()).size(BAR_TEXT)
}

/// 열려 있는 버튼의 강조색(노랑 배경 + 검은 글자) — 한눈에 "지금 이게 떠 있다"를 보여준다.
fn active_text(s: impl Into<String>) -> egui::RichText {
    bar_text(s).color(egui::Color32::BLACK).strong()
}

impl crate::tabs::TermTabViewer<'_> {
    /// pane에 AI 명령 바를 그린다(설정 꺼짐/비AI pane이면 아무것도 안 그림).
    pub(crate) fn ai_bar(&mut self, ui: &mut egui::Ui, pane: nabi_types::PaneId) {
        if !self.ai_cmd_bar {
            return;
        }
        // 화면 판독(모드·모델·노력·제목)은 내용이 바뀐 프레임에만 — 결과는 pane별 캐시.
        let scr = self.ai_screen_state(pane);
        // CLI 종류: 셸 통합이 있으면 실행 명령으로, 없으면(=SSH pane) **창 제목**으로 판정한다.
        let Some(kind) = self
            .run_cmd
            .get(&pane)
            .and_then(|c| bar_kind(c))
            .or(scr.title_kind)
        else {
            return;
        };
        let picks = self.ai_picks.get(&pane).cloned().unwrap_or_default();
        // 모델 우선순위: CLI 상태줄(pane_status) → **화면에서 읽은 현재 모델** → 이 pane에서
        // 고른 값 → 설정에 남은 마지막 선택. 화면 판독이 있어야 재시작 후에도 실제 모델이 뜬다.
        let model = self
            .pane_status
            .get(&pane)
            .and_then(|m| m.get("model"))
            .cloned()
            .or(scr.model)
            .or(picks.model)
            .or_else(|| Some(self.ai_last_model.to_owned()).filter(|s| !s.is_empty()));
        let effort = scr
            .effort
            .or(picks.effort)
            .or_else(|| Some(self.ai_last_effort.to_owned()).filter(|s| !s.is_empty()));
        let view = BarView {
            kind,
            mode: scr.mode,
            model: model.as_deref(),
            effort: effort.as_deref(),
            active: picks.active.as_deref(),
        };
        let Some(action) = show_bar(ui, self.lang, &view) else { return };
        let data = match action {
            BarAction::Cmd(cmd, opens_ui) => {
                let entry = self.ai_picks.entry(pane).or_default();
                // 값 선택(예: "/model opus")은 즉시 적용 → 기억하고 설정에도 남긴다.
                if let Some(v) = cmd.strip_prefix("/model ") {
                    entry.model = Some(v.to_owned());
                    *self.ai_pick_out = Some(("model".into(), v.to_owned()));
                } else if let Some(v) = cmd.strip_prefix("/effort ") {
                    entry.effort = Some(v.to_owned());
                    *self.ai_pick_out = Some(("effort".into(), v.to_owned()));
                }
                // 화면을 여는 명령이면 활성으로 표시(다시 누르면 ESC로 닫는다).
                entry.active = opens_ui.then(|| cmd.clone());
                let mut d = cmd.into_bytes();
                d.push(b'\r');
                d
            }
            BarAction::Esc => {
                self.ai_picks.entry(pane).or_default().active = None;
                vec![0x1b]
            }
            BarAction::ShiftTab => crate::aimode::SHIFT_TAB.to_vec(),
        };
        self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
    }

    /// 사용자가 pane에 직접 입력하면 바가 표시하던 "열림" 상태를 해제한다 —
    /// 화면을 키보드로 닫았을 수 있으므로 바만 계속 노랗게 남지 않게 한다.
    pub(crate) fn clear_ai_active(&mut self, pane: nabi_types::PaneId) {
        if let Some(p) = self.ai_picks.get_mut(&pane) {
            p.active = None;
        }
    }

    /// pane 화면 판독 결과(모드·모델·노력·제목 종류) — 세대가 같으면 캐시를 그대로 쓴다.
    fn ai_screen_state(&mut self, pane: nabi_types::PaneId) -> crate::aimode::AiScreen {
        let Some(md) = self.orch.panes.read().ok().and_then(|m| m.get(&pane).map(|v| v.model.clone()))
        else {
            return crate::aimode::AiScreen { mode: "aimode.unknown", ..Default::default() };
        };
        let Ok(model) = md.lock() else {
            return crate::aimode::AiScreen { mode: "aimode.unknown", ..Default::default() };
        };
        let gen = model.render_gen();
        if let Some(c) = self.ai_screen.get(&pane) {
            if c.gen == gen {
                return c.clone();
            }
        }
        let scanned = crate::aimode::scan(&model, gen);
        self.ai_screen.insert(pane, scanned.clone());
        scanned
    }
}

/// 바 한 줄을 그리는 데 필요한 현재 상태.
pub(crate) struct BarView<'a> {
    pub kind: &'static str,
    pub mode: &'static str,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub active: Option<&'a str>,
}

/// 명령 바 UI. 반환 = 고른 동작.
pub(crate) fn show_bar(ui: &mut egui::Ui, lang: nabi_i18n::Lang, v: &BarView) -> Option<BarAction> {
    let mut send = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // 모든 버튼이 같은 여백을 쓰게 고정 — 드롭다운/일반 버튼 높이 일치.
        ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
        ui.label(bar_text(format!("\u{1f916} {}", v.kind)).color(crate::theme_ui::ACCENT));
        // 모드 순환 버튼: 누를 때마다 shift+tab을 보내고, 현재 모드를 화면에서 읽어 보여준다.
        if ui
            .button(bar_text(format!("\u{21e7}\u{21e5} {}", tr(lang, v.mode))))
            .on_hover_text(tr(lang, "aicb.shifttab"))
            .clicked()
        {
            send = Some(BarAction::ShiftTab);
        }
        ui.separator();
        for bc in primary_commands(v.kind) {
            // /model·/effort는 명령 이름 대신 현재 값을 노출(사용자 요청) — 없으면 요약명.
            let cur = match bc.cmd {
                "/model" => v.model,
                "/effort" => v.effort,
                _ => None,
            };
            let label = cur.map_or_else(|| tr(lang, bc.label).to_string(), str::to_string);
            let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
            let open = v.active == Some(bc.cmd);
            if open {
                // 열려 있는 동안은 강조색 + 다시 누르면 ESC로 닫는다(드롭다운도 이때는 닫기 버튼).
                let btn = egui::Button::new(active_text(label)).fill(crate::theme_ui::BROADCAST);
                if ui.add(btn).on_hover_text(tr(lang, "aicb.close")).clicked() {
                    send = Some(BarAction::Esc);
                }
            } else if bc.sub.is_empty() {
                if ui.button(bar_text(label)).on_hover_text(tip).clicked() {
                    send = Some(BarAction::Cmd(bc.cmd.to_string(), bc.opens_ui));
                }
            } else {
                ui.menu_button(bar_text(format!("{label} \u{25be}")), |ui| {
                    for (sub_label, cmd) in bc.sub {
                        if ui.button(*sub_label).clicked() {
                            // 값이 붙지 않은 순수 명령만 화면을 연다(값 선택은 즉시 적용).
                            send = Some(BarAction::Cmd((*cmd).to_string(), *cmd == bc.cmd));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tip);
            }
        }
        ui.menu_button(bar_text("\u{22ef}"), |ui| {
            for bc in secondary_commands(v.kind) {
                let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
                if ui.button(tr(lang, bc.label)).on_hover_text(tip).clicked() {
                    send = Some(BarAction::Cmd(bc.cmd.to_string(), bc.opens_ui));
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(tr(lang, "aicb.more"));
    });
    send
}
