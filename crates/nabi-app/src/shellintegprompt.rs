//! 셸 통합 설치 권장 모달 — 시작 시 미설치면 1회 띄워 한 번에 설치할 수 있게 한다.
//! (셸 통합이 있어야 종료 시 실행 중 명령·cwd 복원, 종료코드·프롬프트 점프가 동작.)

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 시작 시 호출: 미설치 + "다시 보지 않기" 아님이면 권장 모달을 띄운다.
    pub(crate) fn maybe_prompt_shellinteg(&mut self) {
        if !self.config.terminal.shellinteg_dismissed && !crate::shellinteg::is_installed() {
            self.shellinteg_prompt = true;
        }
    }

    /// 셸 통합 설치 권장 모달(설치 / 나중에 / 다시 보지 않기).
    pub(crate) fn show_shellinteg_prompt(&mut self, ctx: &egui::Context) {
        if !self.shellinteg_prompt {
            return;
        }
        let lang = self.lang;
        let (mut open, mut install, mut never) = (true, false, false);
        egui::Window::new(tr(lang, "shellinteg.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(440.0); // 본문 줄바꿈.
                ui.label(tr(lang, "shellinteg.why"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "shellinteg.install")).clicked() {
                        install = true;
                    }
                    if ui.button(tr(lang, "shellinteg.later")).clicked() {
                        self.shellinteg_prompt = false;
                    }
                    if ui.button(tr(lang, "shellinteg.never")).clicked() {
                        never = true;
                    }
                });
            });
        if !open {
            self.shellinteg_prompt = false; // X = 이번 세션만 닫기.
        }
        if install {
            let msg = match crate::shellinteg::install() {
                Ok(m) => format!("\u{2713} {m}"),
                Err(e) => format!("\u{2715} {e}"),
            };
            self.notify = Some((msg, std::time::Instant::now()));
            self.shellinteg_prompt = false;
        }
        if never {
            self.config.terminal.shellinteg_dismissed = true;
            self.save_config();
            self.shellinteg_prompt = false;
        }
    }
}
