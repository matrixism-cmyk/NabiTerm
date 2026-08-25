//! 메뉴 액션 실행(menu_bar에서 분리).

use crate::app::NabiApp;
use crate::menu::MenuAction;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn apply(&mut self, ctx: &egui::Context, a: MenuAction) {
        match a {
            MenuAction::Spawn(s) => self.spawn_local(s),
            MenuAction::SpawnAiProfile(i) => self.spawn_ai_profile(i),
            MenuAction::OpenAiProfiles => self.ai_prof_open = true,
            MenuAction::Find => self.find_open = true,
            MenuAction::ResetTerm => self.reset_focused(),
            MenuAction::SendSnippet(cmd) => self.send_snippet(&cmd),
            MenuAction::AddSnippet => {
                if let Some(t) = self.selection_text().map(|s| s.trim().to_string()) {
                    if !t.is_empty() {
                        self.config.terminal.snippets.push(t);
                        let _ = nabi_config::save(&self.config_path, &self.config);
                    }
                }
            }
            MenuAction::RemoveSnippet(i) => {
                if i < self.config.terminal.snippets.len() {
                    self.config.terminal.snippets.remove(i);
                    let _ = nabi_config::save(&self.config_path, &self.config);
                }
            }
            MenuAction::SortSnippets => {
                self.config.terminal.snippets.sort_by_key(|s| s.to_lowercase());
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            MenuAction::ExportSnippets => { let d = self.config.terminal.snippets.join("\n"); self.export_sessions_to(d, "nabi-snippets.txt", "txt", "menu.exportsnippets"); } MenuAction::ImportSnippets => self.import_snippets(),
            MenuAction::CopyLastOutput => self.copy_last_output(ctx),
            MenuAction::ToggleFloatOnTop => self.floating_on_top = !self.floating_on_top,
            MenuAction::Copy => self.copy_selection(ctx),
            MenuAction::Paste => self.paste_to_focused(),
            MenuAction::SelectAll => {
                if let Some(t) = self.select_all_focused() {
                    ctx.copy_text(t);
                }
            }
            MenuAction::ConnectSaved(s) => self.connect_saved(s),
            MenuAction::OpenSftp(s) => self.open_sftp_saved(s, false),
            MenuAction::ImportSshConfig => {
                let path = crate::browser::home_dir().join(".ssh").join("config");
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let imported = crate::sshconfig::parse_ssh_config(&content);
                    self.import_sessions(imported, "menu.importsshconfig", "ssh-config");
                }
            }
            MenuAction::ImportFileZilla => {
                // 설치본을 자동 탐지(%APPDATA%\FileZilla\sitemanager.xml). 없으면 파일 선택 폴백.
                let auto = crate::filezilla::default_path();
                let path = auto.or_else(|| {
                    let mut dlg = rfd::FileDialog::new().add_filter("FileZilla sitemanager", &["xml"]);
                    if let Some(d) = std::env::var_os("APPDATA") {
                        dlg = dlg.set_directory(std::path::Path::new(&d).join("FileZilla"));
                    }
                    dlg.pick_file()
                });
                if let Some(p) = path {
                    if let Ok(b) = std::fs::read(&p) {
                        // 인코딩 자동 감지(BOM/UTF-16/ANSI) — 한글 등 깨짐 방지.
                        let text = crate::editload::decode(&b).0;
                        self.import_sessions(crate::filezilla::parse_filezilla(&text), "menu.importfilezilla", "filezilla");
                    }
                }
            }
            MenuAction::ImportMobaXterm => {
                let path = crate::mobaxterm::default_path()
                    .or_else(|| rfd::FileDialog::new().add_filter("MobaXterm.ini", &["ini"]).pick_file());
                if let Some(p) = path {
                    if let Ok(b) = std::fs::read(&p) {
                        let text = crate::editload::decode(&b).0; // 인코딩 자동 감지(MobaXterm.ini는 UTF-16/ANSI 흔함).
                        self.import_sessions(crate::mobaxterm::parse_mobaxterm(&text), "menu.importmobaxterm", "mobaxterm");
                    }
                }
            }
            MenuAction::ImportXshell => {
                // Xshell 기본 세션 폴더 자동 탐색, 없으면 폴더 선택(한국 1급 — Xshell 이탈 흡수).
                let dir = crate::xshell::default_sessions_dir()
                    .or_else(|| rfd::FileDialog::new().pick_folder());
                if let Some(d) = dir {
                    self.import_sessions(crate::xshell::scan_dir(&d), "menu.importxshell", "xshell");
                }
            }
            MenuAction::BackupAll => {
                let layout = nabi_config::StorageLayout::resolve();
                let text = crate::backup::to_text(&crate::backup::collect(&layout));
                self.export_sessions_to(text, "nabiterm-backup.json", "json", "menu.backupall");
            }
            MenuAction::RestoreAll => {
                // 되돌리기는 되돌릴 수 없다 — 파일을 고르는 것 자체가 확인 절차다.
                let picked = rfd::FileDialog::new().add_filter("nabiTerm backup", &["json"]).pick_file();
                let loaded = picked.and_then(|p| std::fs::read_to_string(p).ok());
                let msg = match loaded.as_deref().and_then(crate::backup::from_text) {
                    Some(b) => match crate::backup::restore(&b, &nabi_config::StorageLayout::resolve()) {
                        Ok(n) => format!("{} ({n}) \u{2713}", tr(self.lang, "menu.restoreall")),
                        Err(e) => format!("\u{2715} {e}"),
                    },
                    None => format!("\u{2715} {}", tr(self.lang, "menu.restoreall.bad")),
                };
                self.notify = Some((msg, std::time::Instant::now()));
            }
            MenuAction::OpenImportScreen => self.open_import_screen(),
            MenuAction::ImportWinScp => {
                // WinSCP는 설치 방식에 따라 레지스트리 또는 WinSCP.ini에 둔다 — 둘 다 찾아본다.
                let text = crate::winscp::find_config().or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("WinSCP.ini / .reg", &["ini", "reg"])
                        .pick_file()
                        .and_then(|p| std::fs::read(p).ok())
                        .map(|b| crate::editload::decode(&b).0)
                });
                if let Some(t) = text {
                    self.import_sessions(crate::winscp::parse(&t), "menu.importwinscp", "winscp");
                }
            }
            MenuAction::ImportPuTTY => {
                // PuTTY는 세션을 레지스트리에 둔다 — reg.exe로 export해 파싱. 실패 시 .reg 파일 선택.
                let text = crate::putty::export_registry_text().or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("PuTTY .reg", &["reg"])
                        .pick_file()
                        .and_then(|p| std::fs::read(p).ok())
                        .map(|b| crate::editload::decode(&b).0)
                });
                if let Some(t) = text {
                    self.import_sessions(crate::putty::parse_putty_reg(&t), "menu.importputty", "putty");
                }
            }
            MenuAction::ExportFileZilla => {
                let data = crate::filezilla::to_sitemanager(&self.sessions.sessions);
                self.export_sessions_to(data, "sitemanager.xml", "xml", "menu.exportfilezilla");
            }
            MenuAction::ExportPuTTY => {
                let data = crate::putty::to_putty_reg(&self.sessions.sessions);
                self.export_sessions_to(data, "putty-sessions.reg", "reg", "menu.exportputty");
            }
            MenuAction::ExportMobaXterm => {
                let data = crate::mobaxterm::to_ini(&self.sessions.sessions);
                self.export_sessions_to(data, "MobaXterm.ini", "ini", "menu.exportmobaxterm");
            }
            MenuAction::ExportSshConfig => {
                let data = crate::sshconfig::to_ssh_config(&self.sessions.sessions);
                self.export_sessions_to(data, "config", "", "menu.exportsshconfig");
            }
            MenuAction::DedupSessions => {
                let n = self.sessions.dedup();
                self.save_sessions();
                let label = tr(self.lang, "menu.dedupsessions");
                self.notify = Some((format!("{label} -{n}"), std::time::Instant::now()));
            }
            MenuAction::SortSessions => {
                self.sessions.sort();
                self.save_sessions();
                self.notify = Some((tr(self.lang, "menu.sortsessions").to_string(), std::time::Instant::now()));
            }
            MenuAction::SortSessionsByHost => {
                self.sessions.sort_by_host();
                self.save_sessions();
                self.notify = Some((tr(self.lang, "menu.sortbyhost").to_string(), std::time::Instant::now()));
            }
            MenuAction::DuplicateSession(s) => { let mut dup = s.clone(); dup.name = self.sessions.unique_copy_name(&s.name); self.sessions.add(dup); self.save_sessions(); }
            MenuAction::EditSession(s) => self.edit_session(&s),
            MenuAction::NewSshConnection => self.new_ssh_connection(),
            // 즉시 지우지 않고 한 번 묻는다 — ✕가 ✏ 옆이라 오클릭이 쉽고 되돌릴 수 없다(sessiondel).
            MenuAction::DeleteSession(name) => self.session_delete_ask = Some(name),
            MenuAction::ExportSessions => {
                if let (Ok(json), Some(dir)) =
                    (nabi_session::export::to_json(&self.sessions), self.config_path.parent())
                {
                    let path = dir.join("sessions_export.json");
                    let _ = std::fs::write(&path, json);
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                }
            }
            MenuAction::ImportSessions => {
                if let Some(dir) = self.config_path.parent() {
                    let path = dir.join("sessions_export.json");
                    if let Ok(tree) = std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|s| nabi_session::export::from_json(&s))
                    {
                        let n = tree.sessions.len();
                        self.sessions.sessions.extend(tree.sessions);
                        self.save_sessions();
                        let label = tr(self.lang, "menu.importsessions");
                        self.notify = Some((format!("{label} +{n}"), std::time::Instant::now()));
                    }
                }
            }
            MenuAction::OpenBrowserTab => {
                self.open_browser_tab();
            }
            MenuAction::ToggleSessionsPanel => {
                self.config.appearance.show_sessions_panel =
                    !self.config.appearance.show_sessions_panel;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            MenuAction::ToggleQcBar => {
                self.config.appearance.show_quickconnect_bar =
                    !self.config.appearance.show_quickconnect_bar;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            MenuAction::ToggleAiDashboard => self.ai_dash_open = !self.ai_dash_open,
            MenuAction::ToggleAiCmdBar => {
                self.config.terminal.ai_cmd_bar = !self.config.terminal.ai_cmd_bar;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            MenuAction::OpenNabiPad => self.open_empty_pad(),
            MenuAction::MoveSessionToGroup(name, folder) => self.set_session_folder(&name, folder),
            MenuAction::SetSessionTag(name, tag) => self.set_session_tag(&name, tag),
            MenuAction::RenameGroup(old, new) => self.rename_folder(&old, &new),
            MenuAction::DisbandGroup(f) => self.rename_folder(&f, ""),
            MenuAction::OpenKeygen => self.keygen = Some(crate::sshkeygenui::KeygenState::new()),
            MenuAction::OpenEnvMgr => self.open_env_mgr(),
            MenuAction::OpenCmdHistory => self.open_cmd_history(),
            MenuAction::TestConnection(host, port) => self.test_connection(host, port, ctx),
            MenuAction::TogglePin(name) => {
                let v = &mut self.config.appearance.pinned_sessions;
                match v.iter().position(|x| x == &name) {
                    Some(i) => drop(v.remove(i)),
                    None => v.push(name),
                }
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            MenuAction::EditNote(name) => { let cur = self.config.appearance.session_notes.get(&name).cloned().unwrap_or_default(); self.note_edit = Some((name, cur)); }
            MenuAction::ConnectFolder(f) => {
                // 폴더 내 모든 세션을 한 번에 연결(E7 레이아웃).
                for s in self.sessions.sessions.clone().into_iter().filter(|s| s.folder.as_deref() == Some(f.as_str())) {
                    self.connect_saved(s);
                }
            }
            MenuAction::TearOff => self.tear_off_focused(),
            MenuAction::DockFloat => self.dock_float_focused(),
            MenuAction::SplitSpawn(s, right) => {
                self.pending_split = Some(right);
                self.spawn_local(s);
            }
            MenuAction::SplitConnect(sess, right) => {
                self.pending_split = Some(right);
                self.connect_saved(sess);
            }
            MenuAction::Arrange(m) => self.pending_arrange = Some(m),
            MenuAction::ToggleBroadcast => self.broadcast = !self.broadcast,
            MenuAction::ToggleOnTop => {
                self.always_on_top = !self.always_on_top;
                self.pending_on_top = Some(self.always_on_top);
                self.config.appearance.always_on_top = self.always_on_top; // 마지막 값 기억.
                let _ = nabi_config::save(&self.config_path, &self.config);
            }

            MenuAction::ToggleFullscreen => {
                self.fullscreen = !self.fullscreen;
                self.pending_fullscreen = Some(self.fullscreen);
            }
            MenuAction::SaveWorkspace => self.save_workspace(),
            MenuAction::RestoreWorkspace => {
                let b = self.dock_browser_panes();
                self.restore_workspace(b);
            }
            MenuAction::OpenConfigDir => {
                if let Some(dir) = self.config_path.parent() {
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                }
            }
            MenuAction::OpenSettings => self.settings_open = true,
            MenuAction::OpenVault => self.vault_unlock_open = true,
            MenuAction::OpenForward => self.open_forward(),
            MenuAction::TileTabs => self.tile_tabs(),
            MenuAction::TabifyTabs => self.tabify_tabs(),
            MenuAction::OpenAbout => self.about_open = true,
            MenuAction::Exit => {
                if self.config.terminal.confirm_close && self.dock.iter_all_tabs().count() > 1 {
                    self.confirm_close = true;
                } else {
                    self.quit();
                }
            }
        }
    }

}
