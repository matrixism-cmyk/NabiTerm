//! "새로워진 점" 창 — 업데이트 뒤 첫 실행에 한 번.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 화면에 넣을 최대 줄 수(전문은 저장소에서 본다).
const MAX_LINES: usize = 40;

impl NabiApp {
    /// 지난 실행과 판이 달라졌는지 보고, 그렇다면 보여 줄 내용을 챙긴다(시작 시 1회).
    pub(crate) fn load_whatsnew(&mut self) {
        let current = env!("CARGO_PKG_VERSION");
        if !crate::whatsnew::should_show(&self.config.appearance.last_run_version, current) {
            // 처음 설치이거나 같은 판 — 조용히 지금 판을 적어 두기만 한다.
            self.remember_version(current);
            return;
        }
        let notes = crate::whatsnew::take(&self.cfg_dir(), current)
            .map(|n| crate::whatsnew::trim_notes(&n, MAX_LINES))
            .filter(|n| !n.trim().is_empty());
        self.whatsnew = Some(notes);
        self.remember_version(current);
    }

    /// 지금 판을 설정에 적어 둔다(다음 실행의 판정 기준).
    fn remember_version(&mut self, current: &str) {
        if self.config.appearance.last_run_version != current {
            self.config.appearance.last_run_version = current.to_string();
            self.save_config();
        }
    }

    /// 창을 그린다. 노트가 없으면 판이 올라갔다는 사실과 저장소 링크만 보여 준다.
    pub(crate) fn show_whatsnew(&mut self, ctx: &egui::Context) {
        let Some(notes) = self.whatsnew.clone() else { return };
        // 온보딩·복구 창과 겹치지 않게 미룬다 — 처음 켠 사람에게 모달 셋은 너무하다.
        if self.onboarding_open || !self.pad_recover.is_empty() {
            return;
        }
        let lang = self.lang;
        let (mut open, mut close) = (true, false);
        egui::Window::new(tr(lang, "whatsnew.title"))
            .open(&mut open)
            .collapsible(false)
            .default_size([620.0, 440.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                ui.add_space(6.0);
                match &notes {
                    Some(text) => {
                        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                            ui.label(text);
                        });
                    }
                    // 노트를 못 챙긴 경우(다른 PC에서 설치본으로 깔았다면 그럴 수 있다).
                    None => {
                        ui.label(tr(lang, "whatsnew.nonotes"));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "whatsnew.releases")).clicked() {
                        crate::paneurl::os_open(nabi_release::RELEASES_URL);
                    }
                    if ui.button(tr(lang, "nabipad.close")).clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
            self.whatsnew = None;
        }
    }
}
