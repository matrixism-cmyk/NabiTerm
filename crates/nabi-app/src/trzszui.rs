//! trzsz 전송의 화면 — 확인 대화상자, pane 위 진행률 오버레이, 취소.
//!
//! 원격이 `trz`/`tsz`를 실행하면 **원격이 우리 디스크를 건드리겠다는 요청**이다. 그래서
//! 언제나 물어본다(자동 수락 없음). 진행률은 터미널에서 시작한 일이라 그 pane 위에 얹는다.

use crate::app::NabiApp;
use crate::xferbar::{xfer_bar, XferView};
use nabi_i18n::tr;
use nabi_proto::{Command, Event, XferDecision, XferMode, XferProgress};
use nabi_trzsz::Rate;
use nabi_types::PaneId;
use std::collections::HashMap;

/// 진행 중인 전송 하나.
pub(crate) struct Live {
    pub progress: XferProgress,
    pub up: bool,
    rate: Rate,
    bps: u64,
    /// 앱이 시작한 뒤 흐른 밀리초 — 속도 계산용(시계를 직접 보지 않는다).
    started: std::time::Instant,
}

#[derive(Default)]
pub(crate) struct TrzszUi {
    /// 사용자에게 물어보는 중인 요청. 답하면 **반드시 비운다** — 그러지 않으면 확인 창이
    /// 전송 내내 떠 있어 화면을 막는다.
    pub ask: Option<(PaneId, XferMode)>,
    /// 수락한 전송의 방향(pane별) — 확인 창을 닫은 뒤에도 화살표를 그려야 한다.
    dir_up: HashMap<PaneId, bool>,
    pub live: HashMap<PaneId, Live>,
}

impl TrzszUi {
    fn on_progress(&mut self, pane: PaneId, p: XferProgress) {
        let up = self.dir_up.get(&pane).copied().unwrap_or(true);
        let e = self.live.entry(pane).or_insert_with(|| Live {
            progress: XferProgress::default(),
            up,
            rate: Rate::default(),
            bps: 0,
            started: std::time::Instant::now(),
        });
        let ms = e.started.elapsed().as_millis() as u64;
        e.bps = e.rate.push(ms, p.done);
        e.progress = p;
    }
}

impl NabiApp {
    /// 오케스트레이터가 보낸 trzsz 이벤트를 받는다.
    pub(crate) fn on_trzsz_event(&mut self, ev: &Event) {
        match ev {
            Event::TrzszAsk { pane, mode } => {
                self.trzsz.ask = Some((*pane, *mode));
            }
            Event::TrzszProgress { pane, progress } => {
                self.trzsz.on_progress(*pane, progress.clone());
            }
            Event::TrzszDone { pane, ok, message, names } => {
                self.trzsz.live.remove(pane);
                self.trzsz.dir_up.remove(pane);
                self.trzsz.ask = None;
                let icon = if *ok { "\u{2705}" } else { "\u{26a0}" };
                let what = if names.is_empty() { message.clone() } else { names.join(", ") };
                self.notify = Some((format!("{icon} {what}"), std::time::Instant::now()));
            }
            _ => {}
        }
    }

    /// 확인 대화상자. 요청이 없으면 아무것도 하지 않는다.
    pub(crate) fn show_trzsz_ask(&mut self, ctx: &egui::Context) {
        let Some((pane, mode)) = self.trzsz.ask else { return };
        let lang = self.lang;
        let mut decision: Option<XferDecision> = None;
        crate::modal::foreground_modal(ctx, "trzsz_ask", |ui| {
            ui.heading(tr(lang, "trzsz.title"));
            ui.label(tr(lang, ask_line(mode)));
            ui.add_space(6.0);
            // 원격이 시작한 일이라는 사실을 감추지 않는다 — 사용자가 이걸 알고 판단해야 한다.
            ui.small(tr(lang, "trzsz.warn"));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let ok = egui::Button::new(
                    egui::RichText::new(tr(lang, accept_label(mode))).color(egui::Color32::WHITE),
                )
                .fill(crate::theme_ui::OK);
                if ui.add(ok).clicked() {
                    decision = choose(pane, mode);
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    decision = Some(XferDecision::reject(pane));
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                decision = Some(XferDecision::reject(pane));
            }
        });
        if let Some(d) = decision {
            // 답했으면 확인 창은 무조건 닫는다. 수락한 경우 방향만 기억해 두었다가
            // pane 오버레이의 화살표에 쓴다(예전에는 ask를 남겨 둬서 전송 내내 창이 떠 있었다).
            if d.accept {
                self.trzsz.dir_up.insert(pane, mode.is_upload());
            }
            self.trzsz.ask = None;
            self.orch.send(Command::TrzszDecide(d));
        }
    }

}

/// pane 위에 얹는 진행률 한 줄(+취소). 누르면 true.
///
/// **자유 함수인 이유**: 탭(tabsterm)과 분리 창(floatterm)이 같은 코드를 써야 한다.
/// 예전에 AI 명령 바를 탭에만 붙였다가 분리 창에서는 아예 안 나온 적이 있다(표면 드리프트).
pub(crate) fn overlay(
    ui: &mut egui::Ui,
    lang: nabi_i18n::Lang,
    state: &TrzszUi,
    pane: PaneId,
) -> bool {
    let Some(l) = state.live.get(&pane) else { return false };
    let mut cancel = false;
    ui.horizontal(|ui| {
        let v = XferView {
            arrow: if l.up { "\u{2b06}" } else { "\u{2b07}" },
            name: &l.progress.name,
            done: l.progress.done,
            total: l.progress.total,
            bps: l.bps,
            index: l.progress.index,
            count: l.progress.count,
            width: (ui.available_width() - 100.0).max(120.0),
        };
        xfer_bar(ui, lang, &v);
        cancel = ui.small_button(format!("\u{2715} {}", tr(lang, "qc.cancel"))).clicked();
    });
    cancel
}

/// 수락 버튼을 누르면 열 대화상자 — 다운로드는 폴더, 업로드는 파일들.
fn choose(pane: PaneId, mode: XferMode) -> Option<XferDecision> {
    match mode {
        XferMode::Download => {
            let dir = rfd::FileDialog::new().pick_folder()?;
            Some(XferDecision::download_to(pane, dir))
        }
        XferMode::Upload => {
            let files = rfd::FileDialog::new().pick_files()?;
            Some(XferDecision::upload(pane, files))
        }
        // `trz -d`는 폴더도 받는다. 파일 선택창으로는 폴더를 고를 수 없으므로 폴더창을 연다
        // (파일만 올리고 싶으면 원격에서 `trz`를 쓰면 된다).
        XferMode::UploadDir => {
            let dir = rfd::FileDialog::new().pick_folder()?;
            Some(XferDecision::upload(pane, vec![dir]))
        }
        // 원격이 올릴 파일을 고르는 모드는 오케스트레이터가 이미 막았다 — 여기까지 오지 않는다.
        XferMode::UploadSpecified => Some(XferDecision::reject(pane)),
    }
}

fn ask_line(mode: XferMode) -> &'static str {
    match mode {
        XferMode::Download => "trzsz.ask.down",
        XferMode::UploadDir => "trzsz.ask.updir",
        _ => "trzsz.ask.up",
    }
}

fn accept_label(mode: XferMode) -> &'static str {
    match mode {
        XferMode::Upload => "trzsz.pickfiles",
        // 받기도 폴더를 고르고, `trz -d`로 올리기도 폴더를 고른다.
        _ => "trzsz.pickfolder",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_says_which_direction() {
        assert_eq!(ask_line(XferMode::Download), "trzsz.ask.down");
        assert_eq!(ask_line(XferMode::Upload), "trzsz.ask.up");
        assert_eq!(ask_line(XferMode::UploadDir), "trzsz.ask.updir");
    }

    #[test]
    fn the_button_asks_for_the_right_thing() {
        assert_eq!(accept_label(XferMode::Download), "trzsz.pickfolder");
        assert_eq!(accept_label(XferMode::Upload), "trzsz.pickfiles");
        assert_eq!(accept_label(XferMode::UploadDir), "trzsz.pickfolder", "trz -d는 폴더를 고른다");
    }

    /// 진행률이 들어오면 속도가 채워지고, 같은 pane은 하나로 합쳐진다.
    #[test]
    fn progress_accumulates_per_pane() {
        let mut ui = TrzszUi::default();
        let p = |done| XferProgress { index: 1, count: 1, name: "a".into(), done, total: 100 };
        ui.on_progress(PaneId(1), p(10));
        ui.on_progress(PaneId(1), p(20));
        assert_eq!(ui.live.len(), 1);
        assert_eq!(ui.live[&PaneId(1)].progress.done, 20);
    }

    /// 확인 창은 답한 뒤 **반드시** 닫혀야 한다 — 안 닫으면 전송 내내 화면을 막는다.
    #[test]
    fn the_prompt_does_not_outlive_the_answer() {
        let mut ui = TrzszUi { ask: Some((PaneId(1), XferMode::Download)), ..Default::default() };
        // show_trzsz_ask 가 하는 일과 같은 상태 전이(그리기는 egui 컨텍스트가 필요하다).
        ui.dir_up.insert(PaneId(1), false);
        ui.ask = None;
        assert!(ui.ask.is_none());
        ui.on_progress(PaneId(1), XferProgress { index: 1, count: 1, name: "a".into(), done: 1, total: 2 });
        assert!(!ui.live[&PaneId(1)].up, "다운로드 화살표가 유지되어야 한다");
    }
}
