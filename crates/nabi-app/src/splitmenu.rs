//! View 메뉴의 분할(Split) 서브메뉴: 셸 또는 저장 세션을 골라 분할로 연다.

use crate::menu::MenuAction;
use nabi_i18n::Lang;
use nabi_session::SavedSession;

/// 한 방향(right=오른쪽/false=아래)의 분할 서브메뉴를 그리고 선택된 액션을 돌려준다.
pub(crate) fn split_menu(
    ui: &mut egui::Ui,
    _lang: Lang,
    sessions: &[SavedSession],
    right: bool,
) -> Option<MenuAction> {
    let mut action = None;
    for (label, kind) in crate::menu::installed_shells() {
        if ui.button(label).clicked() {
            action = Some(MenuAction::SplitSpawn(kind, right));
            ui.close_menu();
        }
    }
    if sessions.iter().any(|s| !s.name.is_empty()) {
        ui.separator();
        for s in sessions {
            if !s.name.is_empty() && ui.button(&s.name).clicked() {
                action = Some(MenuAction::SplitConnect(s.clone(), right));
                ui.close_menu();
            }
        }
    }
    action
}
