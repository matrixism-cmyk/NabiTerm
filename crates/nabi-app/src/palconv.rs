//! 팔레트 명령 → 메뉴 액션 변환.
//!
//! 팔레트와 메뉴는 **같은 기능을 두 이름으로** 부르는 항목이 19개 있었고, 실행 코드도 양쪽에
//! 따로 있었다. 한쪽만 고치면 다른 쪽이 옛 동작으로 남는 전형적인 드리프트 자리다
//! (이 프로젝트가 반복해 겪은 결함 클래스). 여기서 한 번 변환해 **메뉴 구현 하나만** 쓴다.
//!
//! 팔레트에만 있는 명령(LSP·스냅샷·정렬 등)은 `None`을 돌려주고 팔레트 쪽에서 처리한다.

use crate::menu::MenuAction;
use crate::palette::PaletteAction;

/// 메뉴에 같은 기능이 있으면 그 액션으로 바꾼다(없으면 None).
pub(crate) fn to_menu(a: &PaletteAction) -> Option<MenuAction> {
    Some(match a {
        // 이름이 다른 짝 — 팔레트는 "새 …", 메뉴는 "생성" 계열로 불러 왔다.
        PaletteAction::NewLocal(s) => MenuAction::Spawn(s.clone()),
        PaletteAction::NewAiProfile(i) => MenuAction::SpawnAiProfile(*i),
        PaletteAction::AiProfiles => MenuAction::OpenAiProfiles,
        PaletteAction::ConnectSession(s) => MenuAction::ConnectSaved(s.clone()),
        PaletteAction::AiDashboard => MenuAction::ToggleAiDashboard,
        PaletteAction::NewPad => MenuAction::OpenNabiPad,
        PaletteAction::PadInWindow => MenuAction::TogglePadInWindow,
        // 이름이 같은 짝.
        PaletteAction::OpenSftp(s) => MenuAction::OpenSftp(s.clone()),
        PaletteAction::SendSnippet(c) => MenuAction::SendSnippet(c.clone()),
        PaletteAction::CopyLastOutput => MenuAction::CopyLastOutput,
        PaletteAction::DockFloat => MenuAction::DockFloat,
        PaletteAction::TearOff => MenuAction::TearOff,
        PaletteAction::ToggleBroadcast => MenuAction::ToggleBroadcast,
        PaletteAction::ToggleFloatOnTop => MenuAction::ToggleFloatOnTop,
        PaletteAction::OpenKeygen => MenuAction::OpenKeygen,
        PaletteAction::OpenImportScreen => MenuAction::OpenImportScreen,
        PaletteAction::OpenEnvMgr => MenuAction::OpenEnvMgr,
        PaletteAction::OpenWeb => MenuAction::OpenWeb,
        PaletteAction::OpenCmdHistory => MenuAction::OpenCmdHistory,
        PaletteAction::OpenSupportBundle => MenuAction::OpenSupportBundle,
        PaletteAction::CopyCommandBlock => MenuAction::CopyCommandBlock,
        PaletteAction::CheckAllReachable => MenuAction::CheckAllReachable,
        PaletteAction::ReopenClosedDoc => MenuAction::ReopenClosedDoc,
        PaletteAction::OpenForward => MenuAction::OpenForward,
        PaletteAction::OpenSettings => MenuAction::OpenSettings,
        PaletteAction::OpenVault => MenuAction::OpenVault,
        PaletteAction::ResetTerm => MenuAction::ResetTerm,
        PaletteAction::SaveWorkspace => MenuAction::SaveWorkspace,
        // 어디서나 되는 전역 동작 — 메뉴에만 있던 것을 팔레트에서도 부를 수 있게 잇는다.
        PaletteAction::Fullscreen => MenuAction::ToggleFullscreen,
        PaletteAction::TileTabs => MenuAction::TileTabs,
        PaletteAction::MergeTabs => MenuAction::TabifyTabs,
        PaletteAction::ToggleQcBar => MenuAction::ToggleQcBar,
        PaletteAction::ToggleAiCmdBar => MenuAction::ToggleAiCmdBar,
        PaletteAction::About => MenuAction::OpenAbout,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **포괄 가지(`_ => None`)가 이것들을 삼키면 안 된다.**
    ///
    /// 이 파일은 맨 아래에 "나머지는 팔레트가 알아서"라는 가지를 두고 있다. 새 항목을
    /// 그 아래에 적으면 조용히 아무 일도 안 하는 명령이 된다 — 팔레트에는 뜨는데 눌러도
    /// 반응이 없다. 컴파일러는 이것을 못 잡으므로(도달 불가 경고가 안 난다) 시험이 지킨다.
    #[test]
    fn the_global_actions_really_reach_the_menu_implementation() {
        // 두 열거형 다 Debug 를 안 붙였으므로(큰 타입이라 굳이 늘리지 않는다) 이름은
        // 손으로 적어 둔다. 어느 항목이 끊겼는지 실패 메시지에서 바로 보여야 한다.
        let pairs: [(&str, PaletteAction, MenuAction); 6] = [
            ("Fullscreen", PaletteAction::Fullscreen, MenuAction::ToggleFullscreen),
            ("TileTabs", PaletteAction::TileTabs, MenuAction::TileTabs),
            ("MergeTabs", PaletteAction::MergeTabs, MenuAction::TabifyTabs),
            ("ToggleQcBar", PaletteAction::ToggleQcBar, MenuAction::ToggleQcBar),
            ("ToggleAiCmdBar", PaletteAction::ToggleAiCmdBar, MenuAction::ToggleAiCmdBar),
            ("About", PaletteAction::About, MenuAction::OpenAbout),
        ];
        for (name, p, want) in pairs {
            let got = to_menu(&p);
            assert!(
                matches!(&got, Some(m) if std::mem::discriminant(m) == std::mem::discriminant(&want)),
                "{name} 이 메뉴 구현에 닿지 않는다 — 포괄 가지가 삼켰다"
            );
        }
    }

    /// 팔레트에만 있는 명령은 여기서 None 이어야 한다 — 그쪽에서 처리하기 때문이다.
    #[test]
    fn palette_only_commands_stay_none() {
        assert!(to_menu(&PaletteAction::WorktreeList).is_none());
    }
}
