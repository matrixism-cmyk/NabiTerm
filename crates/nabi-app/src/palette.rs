//! 커맨드 팔레트(Ctrl+Shift+P): 메뉴 액션을 퍼지 검색으로 즉시 실행.
//!
//! 메뉴 액션을 라벨 목록으로 노출하고, 부분 문자열 필터 + Enter(첫 결과)/클릭으로 실행한다.

use crate::app::NabiApp;
use nabi_i18n::{tr, Lang};
use nabi_proto::ShellKind;
use std::path::PathBuf;

pub(crate) enum PaletteAction {
    NewLocal(ShellKind),
    OpenRecentFile(PathBuf),
    ConnectSession(nabi_session::SavedSession),
    OpenSftp(nabi_session::SavedSession),
    DuplicateTab,
    ReopenClosed,
    CloseOthers,
    SelectAll,
    ZoomPane,
    PrevPrompt,
    NextPrompt,
    ResetTerm,
    QuickConnect,
    ToggleBroadcast,
    TearOff,
    DockFloat,
    ArrangeTile,
    ArrangeCascade,
    ToggleBrowser,
    OpenBrowserTab,
    ToggleSessionsPanel,
    ToggleStatusBar,
    OpenSettings,
    OpenTelegram,
    OpenVault,
    OpenKnownHosts,
    SaveOutput, EditScrollback,
    OpenForward,
    SaveWorkspace,
    RestoreWorkspace,
    OpenConfigDir,
    ToggleOnTop,
    ZoomIn,
    ZoomOut,
    SetLang(Lang),
    SendSnippet(String),
    AiDashboard, ToggleFloatOnTop, CopyLastOutput, JumpDir(String), QuickSelect, RunHistory(String), PasteClip(String), FocusPane(nabi_types::PaneId), DuplicateConnection, ToggleSessionLog, NewTabHere, ClearBuffer, SyncUpload, SyncDownload, CopyOutputMd, CompareFiles, FindDuplicates, FindLargeFiles, CopySshCmd, GenSshKey, InstallPubkey, SelToPad, CopyTabsMd, SaveAllDocs, NewPad, OpenFileDialog, ScrollBottom, ScrollTop, ReplaceInFiles, DirTree, DirStats,
}

impl NabiApp {
    pub(crate) fn show_command_palette(&mut self, ctx: &egui::Context) {
        // Ctrl+Shift+P 토글, Esc 닫기.
        let toggle = ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::P));
        if toggle {
            self.palette_open = !self.palette_open;
            self.palette_query.clear();
        }
        if !self.palette_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.palette_open = false;
            return;
        }

        let lang = self.lang;
        let mut cmds = crate::palettecmds::palette_commands(lang, &self.sessions, &self.editor_config.recent_files, &self.config.terminal.snippets, &self.config.terminal.dir_visits, &self.config.terminal.cmd_history, &self.clip_history);
        if let Ok(ps) = self.orch.panes.read() { for (p, v) in ps.iter() { if Some(*p) != self.sftp_pane && !self.sftp_bg.contains_key(p) { cmds.push((format!("\u{2b1c} {}", v.title), PaletteAction::FocusPane(*p))); } } } // F2 열린 pane 전환
        let fcwd = self.focused_pane().and_then(|p| self.cwds.get(&p)).map(|c| crate::workspace::strip_uri_slash(c)).unwrap_or_default(); // F3 컨텍스트 명령(cwd 매칭)
        if !fcwd.is_empty() { for cmd in crate::cmdhist::recent_in_cwd(&self.config.terminal.cmd_history, &fcwd, 15) { let s: String = cmd.chars().take(50).collect(); cmds.push((format!("\u{1f4cd} {s}"), PaletteAction::RunHistory(cmd))); } } // 📍 우선
        let q = self.palette_query.to_lowercase();
        let mut chosen: Option<usize> = None;
        let mut enter = false;
        let mut open = true;

        egui::Window::new(tr(lang, "palette.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .default_width(440.0)
            .show(ctx, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text(tr(lang, "palette.hint"))
                        .desired_width(f32::INFINITY),
                );
                resp.request_focus();
                enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.separator();
                egui::ScrollArea::vertical().id_salt("palette_scroll").max_height(320.0).show(ui, |ui| {
                    let mut first: Option<usize> = None;
                    for (i, (label, _)) in cmds.iter().enumerate() {
                        if !q.is_empty() && !fuzzy_match(&label.to_lowercase(), &q) {
                            continue;
                        }
                        if first.is_none() {
                            first = Some(i);
                        }
                        if ui.selectable_label(false, label).clicked() {
                            chosen = Some(i);
                        }
                    }
                    if enter {
                        if let Some(f) = first {
                            chosen = Some(f);
                        }
                    }
                });
            });

        if let Some(i) = chosen {
            if let Some((_, act)) = cmds.into_iter().nth(i) {
                self.run_palette(ctx, act);
            }
            self.palette_open = false;
            self.palette_query.clear();
        } else if !open {
            self.palette_open = false;
        }
    }

    fn run_palette(&mut self, ctx: &egui::Context, a: PaletteAction) {
        use crate::arrange::ArrangeMode;
        match a {
            PaletteAction::NewLocal(s) => self.spawn_local(s),
            PaletteAction::OpenRecentFile(p) => self.open_editor_local(p),
            PaletteAction::ConnectSession(s) => self.connect_saved(s),
            PaletteAction::OpenSftp(s) => self.open_sftp_saved(s, false),
            PaletteAction::DuplicateTab => self.duplicate_focused(),
            PaletteAction::ReopenClosed => self.reopen_closed(),
            PaletteAction::CloseOthers => self.close_other_tabs(),
            PaletteAction::SelectAll => {
                if let Some(text) = self.select_all_focused() {
                    ctx.copy_text(text);
                }
            }
            PaletteAction::ZoomPane => self.toggle_pane_zoom(),
            PaletteAction::PrevPrompt => self.jump_prompt(false),
            PaletteAction::NextPrompt => self.jump_prompt(true),
            PaletteAction::ResetTerm => self.reset_focused(),
            PaletteAction::QuickConnect => self.open_quick_connect(),
            PaletteAction::ToggleBroadcast => self.broadcast = !self.broadcast,
            PaletteAction::TearOff => self.tear_off_focused(),
            PaletteAction::DockFloat => self.dock_float_focused(),
            PaletteAction::ArrangeTile => self.pending_arrange = Some(ArrangeMode::Tile),
            PaletteAction::ArrangeCascade => self.pending_arrange = Some(ArrangeMode::Cascade),
            PaletteAction::ToggleBrowser => self.toggle_browser(),
            PaletteAction::OpenBrowserTab => {
                self.open_browser_tab();
            }
            PaletteAction::ToggleSessionsPanel => {
                self.config.appearance.show_sessions_panel =
                    !self.config.appearance.show_sessions_panel;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            PaletteAction::ToggleStatusBar => {
                self.config.appearance.show_statusbar = !self.config.appearance.show_statusbar;
            }
            PaletteAction::OpenSettings => self.settings_open = true,
            PaletteAction::OpenTelegram => {
                self.settings_open = true;
                let tg = crate::settingsui::PAGE_KEYS.len() - 1; // 텔레그램은 마지막 페이지.
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings_cat"), tg));
            }
            PaletteAction::OpenVault => self.vault_unlock_open = true,
            PaletteAction::OpenKnownHosts => self.known_hosts_open = true,
            PaletteAction::SaveOutput => self.save_focused_output(), PaletteAction::EditScrollback => self.edit_scrollback_in_pad(),
            PaletteAction::OpenForward => self.open_forward(),
            PaletteAction::SaveWorkspace => self.save_workspace(),
            PaletteAction::RestoreWorkspace => {
                let b = self.dock_browser_panes();
                self.restore_workspace(b);
            }
            PaletteAction::OpenConfigDir => {
                if let Some(dir) = self.config_path.parent() {
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                }
            }
            PaletteAction::ToggleOnTop => {
                self.always_on_top = !self.always_on_top;
                self.pending_on_top = Some(self.always_on_top);
            }
            PaletteAction::ZoomIn => self.set_font_size(self.font_size + 1.0),
            PaletteAction::ZoomOut => self.set_font_size(self.font_size - 1.0),
            PaletteAction::SetLang(l) => self.lang = l,
            PaletteAction::SendSnippet(cmd) => self.send_snippet(&cmd),
            PaletteAction::AiDashboard => self.ai_dash_open = !self.ai_dash_open, PaletteAction::ToggleFloatOnTop => self.floating_on_top = !self.floating_on_top, PaletteAction::CopyLastOutput => self.copy_last_output(ctx), PaletteAction::JumpDir(d) => self.spawn_local_at(d), PaletteAction::QuickSelect => self.quick_select_open = true,
            PaletteAction::RunHistory(cmd) => { if let Some(p) = self.focused_pane() { let mut data = cmd.into_bytes(); data.push(b'\r'); self.orch.send(nabi_proto::Command::WriteInput { pane: p, data: bytes::Bytes::from(data) }); } } PaletteAction::PasteClip(t) => self.paste_text_to_focused(t), PaletteAction::FocusPane(p) => { if let Some(loc) = self.dock.find_tab(&p) { self.dock.set_active_tab(loc); } } PaletteAction::DuplicateConnection => self.duplicate_connection(), PaletteAction::ToggleSessionLog => self.toggle_session_log(), PaletteAction::NewTabHere => self.spawn_here(), PaletteAction::ClearBuffer => { if let Some(v) = self.focused_pane().and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned())) { if let Ok(mut md) = v.model.lock() { md.clear_scrollback(); } } } PaletteAction::SyncUpload => self.sync_upload_diff(), PaletteAction::SyncDownload => self.sync_download_diff(), PaletteAction::CopyOutputMd => self.copy_last_output_md(ctx), PaletteAction::CompareFiles => self.compare_selected(), PaletteAction::FindDuplicates => self.find_duplicates(), PaletteAction::FindLargeFiles => self.find_large_files(), PaletteAction::CopySshCmd => self.copy_ssh_command(ctx), PaletteAction::GenSshKey => self.generate_ssh_key(ctx), PaletteAction::InstallPubkey => self.install_pubkey(), PaletteAction::SelToPad => self.selection_to_pad(), PaletteAction::CopyTabsMd => self.copy_open_tabs_md(ctx), PaletteAction::SaveAllDocs => { let n = self.save_all_docs(); self.notify = Some((format!("{} {n}", tr(self.lang, "cmd.savedall")), std::time::Instant::now())); } PaletteAction::NewPad => self.open_empty_pad(), PaletteAction::OpenFileDialog => { if let Some(fp) = rfd::FileDialog::new().pick_file() { self.open_editor_local(fp); } } PaletteAction::ScrollBottom => self.scroll_focused_bottom(), PaletteAction::ScrollTop => self.scroll_focused_top(), PaletteAction::ReplaceInFiles => self.replace_open = true, PaletteAction::DirTree => self.open_dir_tree(), PaletteAction::DirStats => self.open_dir_stats(),
        }
    }
}

/// 부분순서(서브시퀀스) 매치: needle의 글자가 순서대로 hay에 나타나면 true.
fn fuzzy_match(hay: &str, needle: &str) -> bool {
    let mut it = hay.chars();
    needle.chars().all(|nc| it.any(|hc| hc == nc))
}

/// 팔레트에 노출할 (라벨, 액션) 목록을 현재 언어로 만든다(저장 세션 포함).
#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn fuzzy_subsequence() {
        assert!(fuzzy_match("new tab", "nt"));
        assert!(fuzzy_match("new tab", "newtab"));
        assert!(!fuzzy_match("new tab", "tn"));
        assert!(fuzzy_match("anything", ""));
    }
}
