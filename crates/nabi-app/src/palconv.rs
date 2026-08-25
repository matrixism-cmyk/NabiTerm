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
        // 이름이 같은 짝.
        PaletteAction::OpenSftp(s) => MenuAction::OpenSftp(s.clone()),
        PaletteAction::SendSnippet(c) => MenuAction::SendSnippet(c.clone()),
        PaletteAction::CopyLastOutput => MenuAction::CopyLastOutput,
        PaletteAction::DockFloat => MenuAction::DockFloat,
        PaletteAction::TearOff => MenuAction::TearOff,
        PaletteAction::ToggleBroadcast => MenuAction::ToggleBroadcast,
        PaletteAction::ToggleFloatOnTop => MenuAction::ToggleFloatOnTop,
        PaletteAction::OpenKeygen => MenuAction::OpenKeygen,
        PaletteAction::OpenEnvMgr => MenuAction::OpenEnvMgr,
        PaletteAction::OpenCmdHistory => MenuAction::OpenCmdHistory,
        PaletteAction::OpenSupportBundle => MenuAction::OpenSupportBundle,
        PaletteAction::CopyCommandBlock => MenuAction::CopyCommandBlock,
        PaletteAction::CheckAllReachable => MenuAction::CheckAllReachable,
        PaletteAction::OpenForward => MenuAction::OpenForward,
        PaletteAction::OpenSettings => MenuAction::OpenSettings,
        PaletteAction::OpenVault => MenuAction::OpenVault,
        PaletteAction::ResetTerm => MenuAction::ResetTerm,
        PaletteAction::SaveWorkspace => MenuAction::SaveWorkspace,
        _ => return None,
    })
}
