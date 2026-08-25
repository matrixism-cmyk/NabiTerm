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
    // 명령 팔레트 — 모든 명령의 관문이므로 맨 위에, 묶음 밖에 둔다.
    if item_keys(ui, tr(lang, "palette.title"), "Ctrl+Shift+P") {
        act = Some(ToolsPick::OpenPalette);
    }
    ui.separator();
    // 묶음으로 나눈다 — 예전에는 17개가 한 층에 늘어서 있어서 무엇이 어디 있는지 외워야 했다.
    // 기준은 기능의 종류가 아니라 **사용자가 하려는 일**이다(도구 메뉴 정리, 2026-08-25).
    for (group, items) in GROUPS {
        ui.menu_button(tr(lang, group), |ui| {
            for (key, a) in *items {
                if ui.button(tr(lang, key)).clicked() {
                    act = Some(ToolsPick::Pal(a.clone()));
                    ui.close();
                }
            }
        });
    }
    ui.separator();
    if let Some(a) = snippets_menu(ui, lang, snippets) {
        act = Some(ToolsPick::Menu(a));
    }
    act
}

/// 도구 메뉴의 묶음 — (묶음 이름 키, 항목들). 순서가 화면 순서다.
///
/// 한 묶음이 대여섯을 넘으면 다시 나눈다. 두 단계를 넘기지 않는 것이 규칙이라
/// 세 번째 층이 필요해지는 순간은 묶음을 잘못 나눈 것이다.
type Group = (&'static str, &'static [(&'static str, PaletteAction)]);

const GROUPS: &[Group] = &[
    // 이 PC와 서버를 갖추는 일 + 문제를 물어볼 때 필요한 것.
    (
        "tools.grp.setup",
        &[
            ("env.title", PaletteAction::OpenEnvMgr),
            ("keygen.title", PaletteAction::OpenKeygen),
            ("help.agent.title", PaletteAction::OpenAiCli),
            ("bundle.title", PaletteAction::OpenSupportBundle),
        ],
    ),
    // 지나간 것을 되찾는 일.
    (
        "tools.grp.history",
        &[
            ("cmdhist.title", PaletteAction::OpenCmdHistory),
            ("sftp.history", PaletteAction::XferHistory),
            ("bcast.results", PaletteAction::BroadcastResults),
        ],
    ),
    // 지금 화면·작업 공간을 다루는 일(스냅샷은 이 화면을 담는 것이라 여기에 있다).
    (
        "tools.grp.workspace",
        &[
            ("wt.create", PaletteAction::WorktreeCreate),
            ("wt.list", PaletteAction::WorktreeList),
            ("qsel.title", PaletteAction::QuickSelect),
            ("snap.save", PaletteAction::SnapshotSave),
            ("snap.list", PaletteAction::SnapshotList),
        ],
    ),
    // 옮기고 잇는 일.
    (
        "tools.grp.transfer",
        &[
            ("sync.title", PaletteAction::OpenSync),
            ("menu.localforward", PaletteAction::OpenForward),
        ],
    ),
    // 사람 없이 도는 것.
    (
        "tools.grp.automation",
        &[
            ("settings.sec.schedule", PaletteAction::OpenSchedule),
            ("settings.sec.telegram", PaletteAction::OpenTelegram),
        ],
    ),
];

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

#[cfg(test)]
mod group_tests {
    use super::GROUPS;

    /// **두 단계를 넘기지 않는다.** 한 묶음이 커지면 세 번째 층이 필요해지는데, 그건
    /// 묶음을 잘못 나눴다는 뜻이다.
    #[test]
    fn no_group_grows_past_a_glance() {
        for (name, items) in GROUPS {
            assert!(!items.is_empty(), "{name}: 빈 묶음");
            // 한도를 6에서 5로 조였다 — 6에 닿고 나서야 나누면 이미 읽기 불편하다(2026-08-25).
            assert!(items.len() <= 5, "{name}: {}개 — 다시 나눌 때다", items.len());
        }
    }

    /// 같은 항목이 두 묶음에 있으면 사용자는 어느 쪽이 맞는지 알 수 없다.
    #[test]
    fn no_item_appears_twice() {
        let mut seen = std::collections::HashSet::new();
        for (_, items) in GROUPS {
            for (key, _) in *items {
                assert!(seen.insert(*key), "두 묶음에 있다: {key}");
            }
        }
    }

    /// 최상위가 다시 부풀면 정리한 뜻이 없다.
    #[test]
    fn the_top_level_stays_short() {
        assert!(GROUPS.len() <= 6, "묶음이 {}개 — 최상위가 다시 길어졌다", GROUPS.len());
    }

    /// 묶음 이름은 i18n 키여야 한다(그대로 찍히면 영어가 새어 나온다).
    #[test]
    fn group_names_are_translation_keys() {
        for (name, _) in GROUPS {
            assert!(name.starts_with("tools.grp."), "{name}: 키가 아니다");
        }
    }
}
