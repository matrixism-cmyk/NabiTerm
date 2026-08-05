//! 보기(View) 메뉴 본문 — menu.rs에서 분리(라인 한도).

use crate::arrange::ArrangeMode;
use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
use nabi_session::SavedSession;

/// 보기 메뉴 토글들의 현재 적용 상태(메뉴에 색으로 표시).
#[derive(Clone, Copy)]
pub(crate) struct ViewStates {
    pub broadcast: bool,
    pub on_top: bool,
    pub fullscreen: bool,
    pub browser: bool,
    pub sessions_panel: bool,
    pub qcbar: bool,
    pub ai_dash: bool,
    pub float_on_top: bool,
}

/// 토글 라벨 앞에 체크 표시를 붙인다(꺼짐은 같은 폭의 공백 — 글자가 흔들리지 않게).
pub(crate) fn check(on: bool, label: &str) -> String {
    format!("{} {label}", if on { "\u{2713}" } else { "\u{2007}" })
}

/// 체크 표시 + 뒤에 단축키(selectable_label은 shortcut_text를 못 써서 라벨에 붙인다).
/// keys가 비면 단축키 칸 없이 그린다.
fn check_keys(on: bool, label: &str, keys: &str) -> String {
    match keys.is_empty() {
        true => check(on, label),
        false => format!("{}   {keys}", check(on, label)),
    }
}

/// 보기 메뉴 항목들을 그리고 선택된 액션을 돌려준다.
pub(crate) fn view_menu(
    ui: &mut egui::Ui,
    lang: Lang,
    st: ViewStates,
    saved: &[SavedSession],
) -> Option<MenuAction> {
    let (broadcast, on_top, fullscreen) = (st.broadcast, st.on_top, st.fullscreen);
    let mut action = None;
    // 토글 항목은 앞에 ✓를 붙여 켜짐/꺼짐을 분명히 한다. selectable_label의 옅은 배경만으로는
    // 메뉴 안에서 상태를 읽기 어렵다(Windows 메뉴 관례도 체크 표시).
    let mut sel = |ui: &mut egui::Ui, on: bool, key: &str, keys: &str, a: MenuAction| {
        if ui.selectable_label(on, check_keys(on, tr(lang, key), keys)).clicked() {
            action = Some(a);
            ui.close_menu();
        }
    };
    // ── 패널 표시 토글 ──
    sel(ui, st.browser, "menu.browser", "Ctrl+Shift+E", MenuAction::ToggleBrowser);
    sel(ui, st.sessions_panel, "menu.sessionspanel", "", MenuAction::ToggleSessionsPanel);
    sel(ui, st.qcbar, "menu.qcbar", "", MenuAction::ToggleQcBar);
    sel(ui, st.ai_dash, "ai.dashboard", "", MenuAction::ToggleAiDashboard);
    ui.separator();
    // ── 창/탭 배열 ── 분할·분리·배열·탭배열을 한 "배열" 서브메뉴로 묶어 최상위를 간결화(F11).
    ui.menu_button(tr(lang, "menu.layout"), |ui| {
        ui.menu_button(tr(lang, "menu.splitright"), |ui| {
            if let Some(a) = crate::splitmenu::split_menu(ui, lang, saved, true) { action = Some(a); }
        });
        ui.menu_button(tr(lang, "menu.splitdown"), |ui| {
            if let Some(a) = crate::splitmenu::split_menu(ui, lang, saved, false) { action = Some(a); }
        });
        ui.separator();
        // 분리 2종을 "분리" 서브메뉴로 묶음(탭 컨텍스트와 동일 라벨·힌트 — 드리프트 제거).
        ui.menu_button(tr(lang, "menu.detach"), |ui| {
            if ui.button(tr(lang, "tab.tearoff")).on_hover_text(tr(lang, "tab.tearoff.hint")).clicked() { action = Some(MenuAction::TearOff); ui.close_menu(); }
            if ui.button(tr(lang, "tab.dockfloat")).on_hover_text(tr(lang, "tab.dockfloat.hint")).clicked() { action = Some(MenuAction::DockFloat); ui.close_menu(); }
            if ui.selectable_label(st.float_on_top, check(st.float_on_top, tr(lang, "float.ontop"))).clicked() { action = Some(MenuAction::ToggleFloatOnTop); ui.close_menu(); }
        });
        ui.menu_button(tr(lang, "menu.arrange"), |ui| {
            if ui.button(tr(lang, "arrange.tile")).clicked() { action = Some(MenuAction::Arrange(ArrangeMode::Tile)); ui.close_menu(); }
            if ui.button(tr(lang, "arrange.cascade")).clicked() { action = Some(MenuAction::Arrange(ArrangeMode::Cascade)); ui.close_menu(); }
        });
        ui.menu_button(tr(lang, "menu.arrangetabs"), |ui| {
            if ui.button(tr(lang, "tab.tile")).clicked() { action = Some(MenuAction::TileTabs); ui.close_menu(); }
            if ui.button(tr(lang, "tab.merge")).clicked() { action = Some(MenuAction::TabifyTabs); ui.close_menu(); }
        });
    });
    ui.separator();
    // ── 모드 토글 ──
    if ui
        .selectable_label(broadcast, check_keys(broadcast, tr(lang, "menu.broadcast"), "Ctrl+Shift+M"))
        .clicked()
    {
        action = Some(MenuAction::ToggleBroadcast);
        ui.close_menu();
    }
    if ui.selectable_label(on_top, check(on_top, tr(lang, "menu.ontop"))).clicked() {
        action = Some(MenuAction::ToggleOnTop);
        ui.close_menu();
    }
    if ui
        .selectable_label(fullscreen, check_keys(fullscreen, tr(lang, "menu.fullscreen"), "F11"))
        .clicked()
    {
        action = Some(MenuAction::ToggleFullscreen);
        ui.close_menu();
    }
    // 언어 선택은 설정 > 동작(영속)과 팔레트로 이동 — 메뉴 중복 제거.
    action
}
