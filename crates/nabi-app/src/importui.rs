//! **가져오기 한 화면** — 이 PC에서 가져올 수 있는 것을 먼저 보여 준다.
//!
//! 임포터가 흩어져 있으면 무엇을 쓸 수 있는지 알려면 하나씩 눌러 봐야 한다. 여기서는
//! 훑어 본 결과를 한자리에 놓는다 — 찾은 것은 위에, 못 찾은 것은 아래에 흐리게.
//!
//! 못 찾은 것도 **지우지 않고 남긴다.** 목록에서 사라지면 "우리가 그걸 지원하는지"조차
//! 알 수 없다. 대신 눌러서 파일을 직접 고를 수 있다 — 기존 가져오기 동작이 그렇게 만들어져
//! 있다(자동 탐지 실패 시 파일 선택 폴백).

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 가져오기 화면을 연다(세션 메뉴·팔레트). 열 때 한 번 훑는다.
    pub(crate) fn open_import_screen(&mut self) {
        self.import_screen = Some(crate::importscan::scan());
    }

    /// 화면을 그린다. 고른 원본의 기존 가져오기 동작을 그대로 실행한다.
    pub(crate) fn show_import_screen(&mut self, ctx: &egui::Context) {
        if self.import_screen.is_none() {
            return;
        }
        let lang = self.lang;
        let (mut open, mut pick, mut rescan) = (true, None, false);
        // 목록을 잠깐 꺼내 둔다 — 그리는 동안 self를 또 빌릴 수 없다.
        let sources = self.import_screen.take().unwrap_or_default();
        egui::Window::new(tr(lang, "import.title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label(tr(lang, "import.hint"));
                ui.add_space(8.0);
                for (i, s) in sources.iter().enumerate() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let mark = if s.found { "\u{2713}" } else { "\u{2022}" };
                            let color = if s.found { crate::theme_ui::OK } else { crate::theme_ui::MENU_FILL };
                            ui.colored_label(color, mark);
                            ui.strong(s.name);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let label = if s.found { "import.take" } else { "import.pick" };
                                if ui.button(tr(lang, label)).clicked() {
                                    pick = Some(i);
                                }
                            });
                        });
                        // 어디에서 찾았는지 적는다 — 엉뚱한 프로필에서 가져오는 사고를 막는다.
                        match s.found {
                            true => {
                                ui.indent(("w", i), |ui| ui.weak(&s.where_));
                            }
                            false => {
                                ui.indent(("n", i), |ui| ui.weak(tr(lang, "import.notfound")));
                            }
                        }
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(4.0);
                if ui.button(tr(lang, "logview.reload")).clicked() {
                    rescan = true;
                }
            });
        let action = pick.and_then(|i| sources.get(i).map(|s| s.action.clone()));
        self.import_screen = open.then_some(sources);
        if rescan {
            self.open_import_screen();
        }
        if let Some(a) = action {
            self.apply(ctx, a);
        }
    }
}
