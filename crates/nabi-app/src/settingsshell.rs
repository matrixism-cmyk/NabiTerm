//! 설정 화면의 **기본 셸 고르기**(배치 AK) — `settingsui` 에서 갈라 냈다(줄 한도).
//!
//! 목록은 메뉴가 쓰는 것과 **같은 것**(`menu::installed_shells`)을 쓴다. 예전에는 여기서
//! 다섯 개를 그냥 늘어놓아서, 스토어판으로 설치되어 실행되지 않는 pwsh 를 기본 셸로 고를
//! 수 있었다. 그러면 탐색기 우클릭도 새 탭도 전부 열리지 않는데, 무엇이 잘못됐는지는
//! 화면 어디에도 나오지 않았다(사용자 보고 2026-08-29).

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};


/// 기본 셸 고르기 — **이 PC 에서 실제로 열리는 것만** 내놓는다(배치 AK).
///
/// `terminal_rows` 가 소프트 한도를 넘어 갈라 냈다. 한도를 맞추려고 설명을 지우지
/// 않는다는 규칙이 있어서, 줄일 것은 코드 쪽이다.
pub(crate) fn shell_row(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.shell"));
    // **이 PC 에서 실제로 열리는 셸만** 내놓는다(배치 AK).
    //
    // 예전에는 다섯 개를 그냥 늘어놓았다. 그래서 스토어판으로 깔려 실행되지 않는 pwsh 를
    // 기본 셸로 고를 수 있었고, 그러면 탐색기 우클릭도 새 탭도 전부 안 열렸다. 무엇이
    // 잘못됐는지는 화면 어디에도 안 나왔다.
    //
    // 목록은 메뉴가 쓰는 것과 **같은 것**을 쓴다. 같은 판단을 두 곳에 두면 언젠가 한쪽만
    // 고쳐지는데, 이 결함이 바로 그렇게 생겼다.
    let usable = crate::menu::installed_shells();
    egui::ComboBox::from_id_salt("set_shell")
        .selected_text(cfg.terminal.default_shell.clone())
        .show_ui(ui, |ui| {
            for (_, kind) in &usable {
                let s = crate::workspace::shell_to_str(kind);
                ui.selectable_value(&mut cfg.terminal.default_shell, s.clone(), s);
            }
        });
    ui.end_row();
}
