//! 도구(Tools) 메뉴 — 흩어져 있던 생산성 도구를 한 지붕으로(T3-1 메뉴 대개편).
//!
//! 팔레트에만 있던 기능(워크트리·빠른 선택·스케줄 등)을 메뉴에도 노출하되, 실행은
//! 기존 `PaletteAction` 디스패처를 재사용한다(SSOT — 메뉴/팔레트 드리프트 방지).
//! 스니펫은 편집 메뉴에서 이사 왔다(편집=텍스트 조작, 도구=생산성 도구라는 구분).

use crate::menu::{item_keys, MenuAction};
use crate::palette::PaletteAction;
use nabi_i18n::{tr, Lang};

/// 도구 메뉴의 선택 결과 — 팔레트 액션 재사용이 기본, 스니펫 관리만 MenuAction.
pub(crate) enum ToolsPick {
    /// 명령 팔레트 열기(팔레트 자신은 PaletteAction이 될 수 없다).
    OpenPalette,
    Pal(PaletteAction),
    Menu(MenuAction),
}

pub(crate) fn tools_menu(ui: &mut egui::Ui, lang: Lang, snippets: &[String]) -> Option<ToolsPick> {
    let mut act = None;
    // 명령 팔레트 — 모든 명령의 관문이므로 맨 위에.
    if item_keys(ui, tr(lang, "palette.title"), "Ctrl+Shift+P") {
        act = Some(ToolsPick::OpenPalette);
    }
    ui.separator();
    // git 워크트리(병렬 에이전트 작업의 표준 패턴) + 화면 빠른 선택.
    for (key, a) in [
        ("wt.create", PaletteAction::WorktreeCreate),
        ("wt.list", PaletteAction::WorktreeList),
        ("qsel.title", PaletteAction::QuickSelect),
        ("snap.save", PaletteAction::SnapshotSave),
        ("sftp.history", PaletteAction::XferHistory),
        ("sync.title", PaletteAction::OpenSync),
        ("keygen.title", PaletteAction::OpenKeygen),
        ("env.title", PaletteAction::OpenEnvMgr),
        ("cmdhist.title", PaletteAction::OpenCmdHistory),
        ("snap.list", PaletteAction::SnapshotList),
        ("bcast.results", PaletteAction::BroadcastResults),
    ] {
        if ui.button(tr(lang, key)).clicked() {
            act = Some(ToolsPick::Pal(a));
            ui.close();
        }
    }
    ui.separator();
    if let Some(a) = snippets_menu(ui, lang, snippets) {
        act = Some(ToolsPick::Menu(a));
    }
    ui.separator();
    // 포워딩·AI 그룹 + 설정 특정 페이지 지름길(스케줄·텔레그램).
    for (i, (key, a)) in [
        ("menu.localforward", PaletteAction::OpenForward),
        ("ai.dashboard", PaletteAction::AiDashboard),
        ("help.agent.title", PaletteAction::OpenAiCli),
        ("settings.sec.schedule", PaletteAction::OpenSchedule),
        ("settings.sec.telegram", PaletteAction::OpenTelegram),
    ]
    .into_iter()
    .enumerate()
    {
        if i == 3 {
            ui.separator();
        }
        if ui.button(tr(lang, key)).clicked() {
            act = Some(ToolsPick::Pal(a));
            ui.close();
        }
    }
    act
}

/// 스니펫 서브메뉴 — 편집 메뉴에서 이동. 관리 동작(정렬/내보내기/가져오기)은
/// 별도 "관리" 서브메뉴 없이 인라인(3단계 위반 해소).
fn snippets_menu(ui: &mut egui::Ui, lang: Lang, snippets: &[String]) -> Option<MenuAction> {
    let mut action = None;
    ui.menu_button(tr(lang, "menu.snippets"), |ui| {
        ui.weak(tr(lang, "snippets.about")); // 무엇인지 안내(자주 쓰는 명령 저장→클릭 실행).
        ui.weak(tr(lang, "snippets.vars")); // 전송 시 치환되는 플레이스홀더 안내.
        ui.separator();
        if ui.button(tr(lang, "menu.addsnippet")).clicked() {
            action = Some(MenuAction::AddSnippet);
            ui.close();
        }
        if snippets.len() > 1 && ui.button(tr(lang, "menu.sortsnippets")).clicked() {
            action = Some(MenuAction::SortSnippets);
            ui.close();
        }
        if !snippets.is_empty() && ui.button(tr(lang, "menu.exportsnippets")).clicked() {
            action = Some(MenuAction::ExportSnippets);
            ui.close();
        }
        if ui.button(tr(lang, "menu.importsnippets")).clicked() {
            action = Some(MenuAction::ImportSnippets);
            ui.close();
        }
        ui.separator();
        if snippets.is_empty() {
            ui.label(tr(lang, "snippets.empty"));
        }
        for (i, snip) in snippets.iter().enumerate() {
            ui.horizontal(|ui| {
                let short: String = snip.chars().take(36).collect();
                if ui.button(short).on_hover_text(snip).clicked() {
                    action = Some(MenuAction::SendSnippet(snip.clone()));
                    ui.close();
                }
                if ui.small_button("\u{2715}").clicked() {
                    action = Some(MenuAction::RemoveSnippet(i));
                    ui.close();
                }
            });
        }
    })
    .response
    .on_hover_text(tr(lang, "snippets.about"));
    action
}
