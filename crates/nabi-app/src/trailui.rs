//! 에이전트 행동 기록 창(배치 AB M1·M2) — 기록은 **볼 수 없으면 없는 것과 같다.**
//!
//! `nabi-control/trail.rs`가 자취를 모으고, 여기서 보여 준다. 모양은 전송 이력 창
//! (`sftphistory.rs`)을 그대로 따랐다 — 같은 종류의 화면이 서로 다르게 생기면 사용자가
//! 매번 다시 익혀야 한다.
//!
//! ## 왜 내보내기가 필요한가
//!
//! "에이전트가 뭘 했느냐"는 대개 **남에게 설명할 때** 필요하다. 화면으로만 보여 주면
//! 그 순간 붙여넣을 수가 없다. 탭으로 나눈 표라 그대로 붙여도 읽히고 표 계산기에도 들어간다.

use crate::app::NabiApp;
use nabi_control::trail::{self, Outcome};
use nabi_i18n::tr;
use std::time::Instant;

impl NabiApp {
    /// 도구 ▸ 기록·이력 ▸ 에이전트 행동.
    pub(crate) fn show_agent_trail(&mut self, ctx: &egui::Context) {
        if !self.agent_trail_open {
            return;
        }
        let lang = self.lang;
        let entries = trail::entries();
        let mut open = true;
        let mut copy: Option<String> = None;
        egui::Window::new(tr(lang, "trail.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 360.0])
            .show(ctx, |ui| {
                if entries.is_empty() {
                    // 비어 있는 것이 정상이다 — 에이전트를 붙이지 않았으면 아무 일도 없다.
                    ui.weak(tr(lang, "trail.empty"));
                    return;
                }
                ui.horizontal(|ui| {
                    ui.weak(format!("{}: {}", tr(lang, "trail.count"), entries.len()));
                    if ui.small_button(tr(lang, "trail.copy")).clicked() {
                        copy = Some(trail::export(&entries));
                    }
                });
                ui.add(egui::Label::new(
                    egui::RichText::new(tr(lang, "trail.nocontent")).weak().small(),
                ).wrap());
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true) // 최신이 아래라 붙어 있게 — 지금 무슨 일이 나는지 본다.
                    .show(ui, |ui| {
                        egui::Grid::new("agent_trail")
                            .num_columns(5)
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for e in &entries {
                                    let (mark, color) = outcome_mark(e.outcome);
                                    ui.colored_label(color, mark);
                                    ui.weak(format!("{}s", e.at_secs));
                                    ui.label(&e.from);
                                    ui.label(e.verb);
                                    match e.bytes {
                                        0 => ui.label(&e.target),
                                        n => ui.label(format!("{} ({})", e.target, crate::humanfmt::human(n as u64))),
                                    };
                                    ui.end_row();
                                }
                            });
                    });
            });
        if let Some(text) = copy {
            ctx.copy_text(text);
            self.notify = Some((tr(lang, "trail.copied").to_string(), Instant::now()));
        }
        if !open {
            self.agent_trail_open = false;
        }
    }

    /// 에이전트 요청이 **처음 막혔을 때 한 번만** 알린다(배치 AB T1).
    ///
    /// "ask" 모드에는 승인 대화상자가 있어 사용자가 안다. 문제는 **"off"** 다 — 그때는
    /// 대화상자도 없이 조용히 막히고, 사용자는 에이전트가 왜 아무것도 못 하는지 모른다.
    ///
    /// 매번 알리지 않는 이유: 자율 에이전트는 막혀도 계속 시도한다. 매번 띄우면 곧
    /// 읽지 않게 되고, 그러면 없느니만 못하다. **처음 한 번**이면 "아, 꺼 뒀지"를 떠올리기에
    /// 충분하다. 자세한 것은 행동 기록 창에서 본다.
    pub(crate) fn notice_first_denial(&mut self) {
        if self.denial_noticed || nabi_control::trail::denied_total() == 0 {
            return;
        }
        self.denial_noticed = true;
        self.notify = Some((tr(self.lang, "trail.denied.first").to_string(), Instant::now()));
    }
}

/// 결과별 표시 — 거부는 눈에 띄어야 한다. 무엇이 막혔는지가 이 화면을 여는 첫 이유다.
fn outcome_mark(o: Outcome) -> (&'static str, egui::Color32) {
    match o {
        Outcome::Allowed => ("\u{2713}", crate::theme_ui::OK),
        Outcome::Approved => ("\u{2713}\u{fe0f}", crate::theme_ui::ACCENT),
        Outcome::Denied => ("\u{26d4}", crate::theme_ui::ERR),
        Outcome::Failed => ("\u{2717}", crate::theme_ui::ERR),
    }
}