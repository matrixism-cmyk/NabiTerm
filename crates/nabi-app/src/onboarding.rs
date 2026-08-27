//! 첫 실행 환영 화면(OOBE, T3-3) — 설정 파일이 없던 사용자에게 언어·기본 셸·글꼴을 묻는다.
//!
//! 상용 제품의 첫인상: 빈 셸만 뜨는 대신 3가지 핵심 선택을 받고 시작한다.
//! 완료 시 설정을 저장하고 고른 셸을 첫 탭으로 띄운다(자동 스폰은 이 동안 보류 — update.rs).

use crate::app::NabiApp;
use nabi_i18n::tr;

/// ShellKind → 설정 문자열(workspace::shell_from_str의 역방향).
pub(crate) fn shell_to_str(kind: &nabi_proto::ShellKind) -> &'static str {
    use nabi_proto::ShellKind as K;
    match kind {
        K::Pwsh => "pwsh",
        K::Cmd => "cmd",
        K::Wsl { .. } => "wsl",
        K::GitBash => "gitbash",
        _ => "powershell",
    }
}

impl NabiApp {
    /// 첫 실행 환영 모달. `onboarding_open`이 켜져 있는 동안 매 프레임 그린다.
    pub(crate) fn show_onboarding(&mut self, ctx: &egui::Context) {
        if !self.onboarding_open {
            return;
        }
        let lang = self.lang;
        let mut start = false;
        crate::modal::foreground_modal(ctx, "nabi_onboarding", |ui| {
                ui.set_min_width(420.0);
                ui.heading(tr(lang, "ob.title"));
                ui.add_space(6.0);
                ui.label(tr(lang, "ob.intro"));
                ui.add_space(10.0);
                egui::Grid::new("ob_grid").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                    // 언어 — 즉시 반영(이 창부터 바뀐 언어로).
                    ui.label(tr(lang, "ob.lang"));
                    ui.horizontal(|ui| {
                        for l in nabi_i18n::Lang::all() {
                            if ui.selectable_label(self.lang == l, l.label()).clicked() {
                                self.lang = l;
                                self.config.appearance.language = match l {
                                    nabi_i18n::Lang::Ko => "ko",
                                    nabi_i18n::Lang::Ja => "ja",
                                    nabi_i18n::Lang::En => "en",
                                }
                                .into();
                            }
                        }
                    });
                    ui.end_row();
                    // 기본 셸 — 설치된 셸만 제시.
                    ui.label(tr(lang, "ob.shell"));
                    let shells = crate::menu::installed_shells();
                    let cur = self.config.terminal.default_shell.clone();
                    let cur_label = shells
                        .iter()
                        .find(|(_, k)| shell_to_str(k) == cur)
                        .map(|(l, _)| l.clone())
                        .unwrap_or_else(|| shells.first().map(|(l, _)| l.clone()).unwrap_or_default());
                    egui::ComboBox::from_id_salt("ob_shell").selected_text(cur_label).show_ui(ui, |ui| {
                        for (label, kind) in &shells {
                            if ui.selectable_label(shell_to_str(kind) == cur, label).clicked() {
                                self.config.terminal.default_shell = shell_to_str(kind).into();
                            }
                        }
                    });
                    ui.end_row();
                    // 글꼴 크기 — 라이브 반영.
                    ui.label(tr(lang, "ob.font"));
                    if ui.add(egui::Slider::new(&mut self.config.appearance.font_size, 8.0..=28.0)).changed() {
                        self.font_size = self.config.appearance.font_size;
                    }
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.weak(tr(lang, "ob.hint")); // 나중에 설정에서 언제든 변경 가능.
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    if ui.add_sized([180.0, 32.0], egui::Button::new(tr(lang, "ob.start"))).clicked() {
                        start = true;
                    }
                });
        });
        if start {
            self.onboarding_open = false;
            self.save_config();
            let shell = crate::workspace::shell_from_str(&self.config.terminal.default_shell);
            self.spawn_local(shell);
        }
    }
}
