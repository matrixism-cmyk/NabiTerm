//! 상단 풀다운 메뉴바(다국어 + 세션 관리).

use crate::app::NabiApp;
use crate::arrange::ArrangeMode;
use nabi_i18n::tr;
use nabi_proto::ShellKind;
use nabi_session::SavedSession;

/// 목록에서 골라 나중에 실행하는 곳이 있어 Clone 이어야 한다(가져오기 한 화면).
#[derive(Clone)]
pub(crate) enum MenuAction {
    Spawn(ShellKind),
    /// AI 터미널 프로필 i번으로 새 터미널(세션▸새 AI 터미널 — aiprof.rs).
    SpawnAiProfile(usize),
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
    /// WinSCP 사이트 가져오기(레지스트리 또는 WinSCP.ini).
    ImportWinScp,
    /// 가져오기 한 화면(설치 자동 탐지).
    OpenImportScreen,
    /// 설정·세션·known_hosts를 한 파일로 백업(볼트 제외).
    BackupAll,
    /// 백업 파일에서 되돌린다(기존 파일은 .bak으로 밀어 둔다).
    RestoreAll,
    ImportXshell,
    ExportFileZilla,
    ExportMobaXterm,
    ExportPuTTY,
    ExportSshConfig,
    DedupSessions,
    SortSessions,
    SortSessionsByHost,
    OpenBrowserTab,
    /// 저장 세션에 표식(운영/개발…)을 지정한다.
    SetSessionTag(String, nabi_session::SessionTag),
    ToggleSessionsPanel,
    ToggleQcBar,
    /// AI 명령 바 표시 토글(terminal.ai_cmd_bar).
    ToggleAiCmdBar,
    ToggleAiDashboard, ConnectFolder(String), OpenNabiPad, MoveSessionToGroup(String, Option<String>),
    RenameGroup(String, String), DisbandGroup(String), OpenKeygen, OpenEnvMgr, TestConnection(String, u16), TogglePin(String), EditNote(String),
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
    /// AI 터미널 프로필 관리 독립창(aiprofileui.rs).
    OpenAiProfiles,
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
    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let mut action: Option<MenuAction> = None;
        let mut tool: Option<crate::toolsmenu::ToolsPick> = None;
        let lang = self.lang;
        let vstates = crate::viewmenu::ViewStates {
            broadcast: self.broadcast,
            on_top: self.always_on_top,
            fullscreen: self.fullscreen,
            sessions_panel: self.config.appearance.show_sessions_panel,
            qcbar: self.config.appearance.show_quickconnect_bar,
            ai_cmd_bar: self.config.terminal.ai_cmd_bar,
            ai_dash: self.ai_dash_open,
            float_on_top: self.floating_on_top,
        };
        // 새 버전이 확인된 상태인가(메뉴 띠 오른쪽 '업데이트' 버튼 노출 조건).
        let update_ready = matches!(self.updater.get_status(), nabi_release::UpdateStatus::Available(_));
        let mut open_update = false;
        let mut saved = self.sessions.sessions.clone();
        saved.sort_by_key(|a| a.name.to_lowercase());
        let snippets = self.config.terminal.snippets.clone(); let last_conn = self.config.terminal.last_connected.clone();
        // 현재 연결된 SSH 출처 집합(세션 목록 라이브 표시 D9).
        let active: std::collections::HashSet<String> = self.pane_origins.values().filter_map(|k| match k {
            nabi_session::SessionKind::Ssh { host, user, port, .. } => Some(format!("{user}@{host}:{port}")), _ => None }).collect();
        let mbar = egui::Frame::NONE
            .fill(crate::theme_ui::MENU_FILL)
            .inner_margin(egui::Margin::symmetric(6, 3));
        egui::Panel::top("menubar").frame(mbar).show(ui, |ui| {
            // 메뉴 띠 글씨를 강제로 밝게(어두운 띠에서 확실히 보이도록).
            ui.visuals_mut().override_text_color = Some(crate::theme_ui::TEXT_BRIGHT);
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(tr(lang, "menu.file"), |ui| {
                    // '새 로컬 터미널'은 세션 메뉴로 통합(2026-08-18 사용자 요청 — 새로 열기 축 일원화).
                    // 파일 브라우저 탭 — "여는" 동작이라 보기 토글이 아닌 파일 메뉴에.
                    if ui.button(tr(lang, "menu.newpad")).clicked() { action = Some(MenuAction::OpenNabiPad); }
                    if ui.button(tr(lang, "menu.browsertab")).clicked() {
                        action = Some(MenuAction::OpenBrowserTab);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(tr(lang, "menu.saveworkspace")).clicked() { action = Some(MenuAction::SaveWorkspace); ui.close(); }
                    if ui.button(tr(lang, "menu.restoreworkspace")).clicked() { action = Some(MenuAction::RestoreWorkspace); ui.close(); }
                    if ui.button(tr(lang, "menu.configdir")).clicked() {
                        action = Some(MenuAction::OpenConfigDir);
                        ui.close();
                    }
                    ui.separator();
                    if item_keys(ui, tr(lang, "menu.exit"), "Ctrl+Shift+Q") {
                        action = Some(MenuAction::Exit);
                    }
                });
                ui.menu_button(tr(lang, "menu.sessions"), |ui| {
                    // 새로 열기 축 통합: 새 로컬 터미널 + 새 AI 터미널(프로필) + 저장 세션 관리.
                    ui.menu_button(tr(lang, "menu.newlocal"), |ui| {
                        for (label, shell) in installed_shells() {
                            if ui.button(label).clicked() {
                                action = Some(MenuAction::Spawn(shell));
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button(tr(lang, "menu.newai"), |ui| {
                        let profiles = &self.config.terminal.ai_profiles;
                        for (i, p) in profiles.iter().enumerate() {
                            let label = format!("{} — {}", p.name, p.cmd);
                            if ui.button(label).clicked() {
                                action = Some(MenuAction::SpawnAiProfile(i));
                                ui.close();
                            }
                        }
                        if profiles.is_empty() {
                            ui.weak(tr(lang, "aiprof.none"));
                        }
                        ui.separator();
                        if ui.button(tr(lang, "aiprof.manage")).clicked() {
                            action = Some(MenuAction::OpenAiProfiles);
                            ui.close();
                        }
                    });
                    ui.separator();
                    // 로컬 포워딩은 sessions_menu → manage_menu(공용)에 포함 — 사이드바 ⋯와 동일.
                    if let Some(a) = crate::sessionsmenu::sessions_menu(ui, lang, &saved, &last_conn, &active) {
                        action = Some(a);
                    }
                });
                ui.menu_button(tr(lang, "menu.edit"), |ui| {
                    if item_keys(ui, tr(lang, "menu.copy"), "Ctrl+Shift+C") {
                        action = Some(MenuAction::Copy);
                        ui.close();
                    }
                    if item_keys(ui, tr(lang, "menu.paste"), "Ctrl+Shift+V") {
                        action = Some(MenuAction::Paste);
                        ui.close();
                    }
                    if item_keys(ui, tr(lang, "menu.selectall"), "Ctrl+Shift+A") {
                        action = Some(MenuAction::SelectAll);
                        ui.close();
                    }
                    if item_keys(ui, tr(lang, "menu.find"), "Ctrl+F") {
                        action = Some(MenuAction::Find);
                        ui.close();
                    }
                    if item_keys(ui, tr(lang, "term.reset"), "Ctrl+Shift+K") {
                        action = Some(MenuAction::ResetTerm);
                        ui.close();
                    }
                    if ui.button(tr(lang, "cmd.copyoutput")).clicked() { action = Some(MenuAction::CopyLastOutput); ui.close(); }
                    // 스니펫은 도구 메뉴로 이동(T3-1 — 편집=텍스트 조작, 도구=생산성 도구).
                });
                ui.menu_button(tr(lang, "menu.view"), |ui| {
                    if let Some(a) =
                        crate::viewmenu::view_menu(ui, lang, vstates, &saved)
                    {
                        action = Some(a);
                    }
                });
                // 도구: 팔레트·워크트리·스니펫·포워딩·AI·스케줄 등 생산성 도구 집합(T3-1 신설).
                ui.menu_button(tr(lang, "menu.tools"), |ui| {
                    if let Some(p) = crate::toolsmenu::tools_menu(ui, lang, &snippets) {
                        tool = Some(p);
                    }
                });
                if ui.button(tr(lang, "menu.settings")).clicked() {
                    action = Some(MenuAction::OpenSettings);
                    ui.close();
                }
                // 볼트는 세션 메뉴(관리 영역)로 흡수 — 최상위 7→6(T3-1).
                if ui.button(tr(lang, "menu.help")).clicked() {
                    action = Some(MenuAction::OpenAbout); // 클릭 즉시 도움말(설정과 동일).
                    ui.close();
                }
                // 새 버전이 있으면 메뉴 띠 오른쪽 끝에 '업데이트' 버튼(updatemodal 소관).
                open_update = crate::updatemodal::update_button(ui, lang, update_ready);
            });
        });
        if open_update {
            self.update_modal = true; // 확인 창(변경 내용 + 재시작 안내)을 연다.
        }
        if let Some(a) = action {
            self.apply(ctx, a);
        }
        if let Some(p) = tool {
            match p {
                crate::toolsmenu::ToolsPick::OpenPalette => {
                    self.palette_open = true;
                    self.palette_query.clear();
                }
                crate::toolsmenu::ToolsPick::Pal(a) => self.run_palette(ctx, a),
                crate::toolsmenu::ToolsPick::Menu(a) => self.apply(ctx, a),
            }
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

/// 실제 설치된 셸만 추린 목록 — 메뉴·팔레트·분할에 노출.
///
/// 한 번 훑어 캐시하지만 **영구는 아니다.** 환경 관리자에서 WSL이나 PowerShell 7을 깔면
/// 그 자리에서 목록에 나타나야 한다(`refresh_shells`). 예전에는 `OnceLock`이라 다시 켜기
/// 전까지 반영되지 않았다.
static SHELL_CACHE: std::sync::RwLock<Option<Vec<(String, ShellKind)>>> = std::sync::RwLock::new(None);

/// 캐시를 버린다 — 다음 호출 때 다시 훑는다(환경 관리자에서 무언가를 깐 뒤).
pub(crate) fn clear_shell_cache() {
    if let Ok(mut c) = SHELL_CACHE.write() {
        *c = None;
    }
}

pub(crate) fn installed_shells() -> Vec<(String, ShellKind)> {
    use SHELL_CACHE as CACHE;
    if let Ok(c) = CACHE.read() {
        if let Some(v) = c.as_ref() {
            return v.clone();
        }
    }
    let v = detect_shells();
    if let Ok(mut c) = CACHE.write() {
        *c = Some(v.clone());
    }
    v
}

/// 이 PC를 실제로 훑는다.
fn detect_shells() -> Vec<(String, ShellKind)> {
    let mut v = Vec::new();
    for (label, kind) in shell_choices() {
        // WSL은 실행 파일 유무로 판단할 수 없다 — wsl.exe는 미설치 PC에도 늘 있다.
        if matches!(kind, ShellKind::Wsl { .. }) {
            v.extend(crate::shelldetect::wsl_entries(&nabi_pty::wsl_distros()));
            continue;
        }
        if nabi_pty::resolve_shell(&kind).is_some() {
            v.push((label.to_string(), kind));
        }
    }
    v.extend(crate::shelldetect::extras(&|p| std::path::Path::new(p).is_file()));
    v
}
