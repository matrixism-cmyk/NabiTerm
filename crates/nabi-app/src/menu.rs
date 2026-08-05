//! 상단 풀다운 메뉴바(다국어 + 세션 관리).

use crate::app::NabiApp;
use crate::arrange::ArrangeMode;
use nabi_i18n::tr;
use nabi_proto::ShellKind;
use nabi_session::SavedSession;

pub(crate) enum MenuAction {
    Spawn(ShellKind),
    Copy,
    Paste,
    SelectAll,
    Find,
    ResetTerm,
    SendSnippet(String),
    AddSnippet,
    SortSnippets,
    ExportSnippets,
    ImportSnippets,
    CopyLastOutput,
    ToggleFloatOnTop,
    RemoveSnippet(usize),
    ConnectSaved(SavedSession),
    OpenSftp(SavedSession),
    DuplicateSession(SavedSession),
    EditSession(SavedSession),
    NewSshConnection,
    DeleteSession(String),
    ExportSessions,
    ImportSessions,
    ImportSshConfig,
    ImportFileZilla,
    ImportMobaXterm,
    ImportPuTTY,
    ExportFileZilla,
    ExportMobaXterm,
    ExportPuTTY,
    ExportSshConfig,
    DedupSessions,
    SortSessions,
    SortSessionsByHost,
    ToggleBrowser,
    OpenBrowserTab,
    ToggleSessionsPanel,
    ToggleQcBar,
    ToggleAiDashboard, ConnectFolder(String), OpenNabiPad, MoveSessionToGroup(String, Option<String>), TestConnection(String, u16), TogglePin(String), EditNote(String),
    TearOff,
    DockFloat,
    Arrange(ArrangeMode),
    TileTabs,
    TabifyTabs,
    ToggleBroadcast,
    ToggleOnTop,
    ToggleFullscreen,
    SplitSpawn(ShellKind, bool),
    SplitConnect(SavedSession, bool),
    SaveWorkspace,
    RestoreWorkspace,
    OpenConfigDir,
    OpenSettings,
    OpenVault,
    OpenForward,
    OpenAbout,
    Exit,
}

/// 단축키가 있는 메뉴 항목 — 오른쪽에 흐린 글씨로 키를 보여 준다.
///
/// 단축키는 shortcuts.rs에 16개나 있는데 메뉴에는 F11만 적혀 있었다. 쓰는 사람이
/// 알 방법이 없으면 없는 기능이나 마찬가지다.
pub(crate) fn item_keys(ui: &mut egui::Ui, label: &str, keys: &str) -> bool {
    ui.add(egui::Button::new(label).shortcut_text(keys)).clicked()
}

impl NabiApp {
    pub(crate) fn menu_bar(&mut self, ctx: &egui::Context) {
        let mut action: Option<MenuAction> = None;
        let lang = self.lang;
        let vstates = crate::viewmenu::ViewStates {
            broadcast: self.broadcast,
            on_top: self.always_on_top,
            fullscreen: self.fullscreen,
            browser: self.browser.open,
            sessions_panel: self.config.appearance.show_sessions_panel,
            qcbar: self.config.appearance.show_quickconnect_bar,
            ai_dash: self.ai_dash_open,
            float_on_top: self.floating_on_top,
        };
        let mut saved = self.sessions.sessions.clone();
        saved.sort_by_key(|a| a.name.to_lowercase());
        let snippets = self.config.terminal.snippets.clone(); let last_conn = self.config.terminal.last_connected.clone();
        // 현재 연결된 SSH 출처 집합(세션 목록 라이브 표시 D9).
        let active: std::collections::HashSet<String> = self.pane_origins.values().filter_map(|k| match k {
            nabi_session::SessionKind::Ssh { host, user, port, .. } => Some(format!("{user}@{host}:{port}")), _ => None }).collect();
        let mbar = egui::Frame::none()
            .fill(crate::theme_ui::MENU_FILL)
            .inner_margin(egui::Margin::symmetric(6.0, 3.0));
        egui::TopBottomPanel::top("menubar").frame(mbar).show(ctx, |ui| {
            // 메뉴 띠 글씨를 강제로 밝게(어두운 띠에서 확실히 보이도록).
            ui.visuals_mut().override_text_color = Some(crate::theme_ui::TEXT_BRIGHT);
            egui::menu::bar(ui, |ui| {
                ui.menu_button(tr(lang, "menu.file"), |ui| {
                    ui.menu_button(tr(lang, "menu.newlocal"), |ui| {
                        for (label, shell) in installed_shells() {
                            if ui.button(label).clicked() {
                                action = Some(MenuAction::Spawn(shell));
                                ui.close_menu();
                            }
                        }
                    });
                    // 파일 브라우저 탭 — "여는" 동작이라 보기 토글이 아닌 파일 메뉴에.
                    if ui.button(tr(lang, "menu.newpad")).clicked() { action = Some(MenuAction::OpenNabiPad); }
                    if ui.button(tr(lang, "menu.browsertab")).clicked() {
                        action = Some(MenuAction::OpenBrowserTab);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr(lang, "menu.saveworkspace")).clicked() { action = Some(MenuAction::SaveWorkspace); ui.close_menu(); }
                    if ui.button(tr(lang, "menu.restoreworkspace")).clicked() { action = Some(MenuAction::RestoreWorkspace); ui.close_menu(); }
                    if ui.button(tr(lang, "menu.configdir")).clicked() {
                        action = Some(MenuAction::OpenConfigDir);
                        ui.close_menu();
                    }
                    ui.separator();
                    if item_keys(ui, tr(lang, "menu.exit"), "Ctrl+Shift+Q") {
                        action = Some(MenuAction::Exit);
                    }
                });
                ui.menu_button(tr(lang, "menu.sessions"), |ui| {
                    // 로컬 포워딩은 sessions_menu → manage_menu(공용)에 포함 — 사이드바 ⋯와 동일.
                    if let Some(a) = crate::sessionsmenu::sessions_menu(ui, lang, &saved, &last_conn, &active) {
                        action = Some(a);
                    }
                });
                ui.menu_button(tr(lang, "menu.edit"), |ui| {
                    if item_keys(ui, tr(lang, "menu.copy"), "Ctrl+Shift+C") {
                        action = Some(MenuAction::Copy);
                        ui.close_menu();
                    }
                    if item_keys(ui, tr(lang, "menu.paste"), "Ctrl+Shift+V") {
                        action = Some(MenuAction::Paste);
                        ui.close_menu();
                    }
                    if item_keys(ui, tr(lang, "menu.selectall"), "Ctrl+Shift+A") {
                        action = Some(MenuAction::SelectAll);
                        ui.close_menu();
                    }
                    if item_keys(ui, tr(lang, "menu.find"), "Ctrl+F") {
                        action = Some(MenuAction::Find);
                        ui.close_menu();
                    }
                    if item_keys(ui, tr(lang, "term.reset"), "Ctrl+Shift+K") {
                        action = Some(MenuAction::ResetTerm);
                        ui.close_menu();
                    }
                    if ui.button(tr(lang, "cmd.copyoutput")).clicked() { action = Some(MenuAction::CopyLastOutput); ui.close_menu(); }
                    ui.menu_button(tr(lang, "menu.snippets"), |ui| {
                        ui.weak(tr(lang, "snippets.about")); // 무엇인지 안내(자주 쓰는 명령 저장→클릭 실행).
                        ui.weak(tr(lang, "snippets.vars")); // 전송 시 치환되는 플레이스홀더 안내.
                        ui.separator();
                        if ui.button(tr(lang, "menu.addsnippet")).clicked() {
                            action = Some(MenuAction::AddSnippet);
                            ui.close_menu();
                        }
                        // 드물게 쓰는 관리 액션(정렬/내보내기/가져오기)은 "관리" 하위로 묶어 비대화 방지(D10).
                        ui.menu_button(tr(lang, "menu.snippetmanage"), |ui| {
                            if snippets.len() > 1 && ui.button(tr(lang, "menu.sortsnippets")).clicked() { action = Some(MenuAction::SortSnippets); ui.close_menu(); }
                            if !snippets.is_empty() && ui.button(tr(lang, "menu.exportsnippets")).clicked() { action = Some(MenuAction::ExportSnippets); ui.close_menu(); }
                            if ui.button(tr(lang, "menu.importsnippets")).clicked() { action = Some(MenuAction::ImportSnippets); ui.close_menu(); }
                        });
                        ui.separator();
                        if snippets.is_empty() {
                            ui.label(tr(lang, "snippets.empty"));
                        }
                        for (i, snip) in snippets.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let short: String = snip.chars().take(36).collect();
                                if ui.button(short).on_hover_text(snip).clicked() {
                                    action = Some(MenuAction::SendSnippet(snip.clone()));
                                    ui.close_menu();
                                }
                                if ui.small_button("\u{2715}").clicked() {
                                    action = Some(MenuAction::RemoveSnippet(i));
                                    ui.close_menu();
                                }
                            });
                        }
                    })
                    .response
                    .on_hover_text(tr(lang, "snippets.about")); // 메뉴 항목 hover 시 설명.
                });
                ui.menu_button(tr(lang, "menu.view"), |ui| {
                    if let Some(a) =
                        crate::viewmenu::view_menu(ui, lang, vstates, &saved)
                    {
                        action = Some(a);
                    }
                });
                if ui.button(tr(lang, "menu.settings")).clicked() {
                    action = Some(MenuAction::OpenSettings);
                    ui.close_menu();
                }
                if ui
                    .button(tr(lang, "menu.vault"))
                    .on_hover_text(tr(lang, "vault.about"))
                    .clicked()
                {
                    action = Some(MenuAction::OpenVault);
                    ui.close_menu();
                }
                if ui.button(tr(lang, "menu.help")).clicked() {
                    action = Some(MenuAction::OpenAbout); // 클릭 즉시 도움말(설정과 동일).
                    ui.close_menu();
                }
            });
        });
        if let Some(a) = action {
            self.apply(ctx, a);
        }
    }

}

/// 로컬 셸 선택지(라벨, ShellKind). 제품명이라 번역하지 않는다.
pub(crate) fn shell_choices() -> [(&'static str, ShellKind); 5] {
    [
        ("Windows PowerShell", ShellKind::WindowsPowerShell),
        ("PowerShell 7 (pwsh)", ShellKind::Pwsh),
        ("Command Prompt", ShellKind::Cmd),
        ("WSL", ShellKind::Wsl { distro: None }),
        ("Git Bash", ShellKind::GitBash),
    ]
}

/// 실제 설치된(실행파일이 PATH에 있는) 셸만 추린 목록 — 메뉴/팔레트에 노출.
/// 한 번만 탐지해 캐시한다(PATH 스캔·WSL 조회 비용 절감). powershell/cmd는 항상 잡힌다.
/// WSL은 설치된 배포판별로 펼친다(여러 개면 "WSL: Ubuntu"처럼; 없으면 기본 "WSL" 1개).
pub(crate) fn installed_shells() -> Vec<(String, ShellKind)> {
    static CACHE: std::sync::OnceLock<Vec<(String, ShellKind)>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut v = Vec::new();
            for (label, kind) in shell_choices() {
                if nabi_pty::resolve_shell(&kind).is_none() {
                    continue;
                }
                if matches!(kind, ShellKind::Wsl { .. }) {
                    let distros = nabi_pty::wsl_distros();
                    if distros.is_empty() {
                        v.push((label.to_string(), kind));
                    } else {
                        for d in distros {
                            v.push((format!("WSL: {d}"), ShellKind::Wsl { distro: Some(d) }));
                        }
                    }
                } else {
                    v.push((label.to_string(), kind));
                }
            }
            v
        })
        .clone()
}
