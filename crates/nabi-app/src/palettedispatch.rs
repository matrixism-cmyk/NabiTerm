//! 팔레트에서 고른 명령을 실제 동작으로 옮긴다(palette.rs의 UI와 분리).
//!
//! 팔레트 항목이 계속 늘어나므로 디스패치만 따로 둔다. 한 항목 = 한 줄이라
//! 새 명령을 넣을 때 어디에 붙일지 고민할 필요가 없다.

use crate::app::NabiApp;
use crate::palette::PaletteAction;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn run_palette(&mut self, ctx: &egui::Context, a: PaletteAction) {
        use crate::arrange::ArrangeMode;
        // 메뉴에도 있는 명령은 메뉴 구현 하나만 쓴다 — 같은 기능을 두 곳에서 고치다
        // 한쪽만 바뀌는 드리프트를 없앤다(19건).
        if let Some(m) = crate::palconv::to_menu(&a) {
            self.apply(ctx, m);
            return;
        }
        match a {
            PaletteAction::OpenRecentFile(p) => self.open_editor_local(p),
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
            PaletteAction::ToggleMark => self.toggle_scroll_mark(),
            PaletteAction::PrevMark => self.jump_scroll_mark(false),
            PaletteAction::NextMark => self.jump_scroll_mark(true),
            PaletteAction::ClearMarks => self.clear_scroll_marks(),
            PaletteAction::SftpFind => self.open_sftp_find(),
            PaletteAction::QuickConnect => self.open_quick_connect(),
            PaletteAction::ArrangeTile => self.pending_arrange = Some(ArrangeMode::Tile),
            PaletteAction::ArrangeCascade => self.pending_arrange = Some(ArrangeMode::Cascade),
            PaletteAction::ToggleBrowser => self.toggle_browser(),
            PaletteAction::FindAll => self.open_find_all(),
            PaletteAction::OpenBrowserTab => {
                self.open_browser_tab(); // 반환값(PaneId)은 쓰지 않는다.
            }
            PaletteAction::ToggleSessionsPanel => {
                let show = &mut self.config.appearance.show_sessions_panel;
                *show = !*show;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            PaletteAction::ToggleStatusBar => {
                self.config.appearance.show_statusbar = !self.config.appearance.show_statusbar;
                let _ = nabi_config::save(&self.config_path, &self.config); // 다른 토글과 동일하게 영속.
            }
            // 스케줄·텔레그램은 한 페이지('자동화')로 합쳌다 — 키로 찾아 눈다(인덱스 하드코딩 금지).
            PaletteAction::OpenTelegram | PaletteAction::OpenSchedule => {
                self.settings_open = true;
                let i = crate::settingsui::page_index("settings.sec.automation");
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings_cat"), i));
            }
            PaletteAction::SnapshotSave => { self.snap_name.clear(); self.snap_save_open = true; }
            PaletteAction::SnapshotList => self.snap_list_open = true,
            PaletteAction::GotoDefinition => self.lsp_goto_definition(),
            PaletteAction::LspHover => self.lsp_hover(),
            PaletteAction::LspRefs => self.lsp_refs(),
            PaletteAction::LspFormat => { if let Some(p) = self.focused_pane() { self.lsp_format_for(p); } }
            PaletteAction::OpenSync => self.open_sync_dialog(),
            PaletteAction::HandoffLast => {
                if let Some(p) = self.focused_pane() {
                    match (self.command_context(p), self.find_ai_pane(p)) {
                        (Some(prompt), Some(ai)) => self.inject_prompt(ai, &prompt),
                        (Some(_), None) => self.notify = Some((nabi_i18n::tr(self.lang, "handoff.noai").to_string(), std::time::Instant::now())),
                        _ => self.notify = Some((nabi_i18n::tr(self.lang, "handoff.nocmd").to_string(), std::time::Instant::now())),
                    }
                }
            }
            PaletteAction::CopyLastMd => {
                if let Some(md) = self.focused_pane().and_then(|p| self.command_markdown(p)) {
                    ctx.copy_text(md);
                } else {
                    self.notify = Some((nabi_i18n::tr(self.lang, "handoff.nocmd").to_string(), std::time::Instant::now()));
                }
            }
            PaletteAction::XferHistory => self.xfer_history_open = true,
            PaletteAction::BroadcastResults => self.bcast_view_open = true,
            PaletteAction::OpenAiCli => {
                self.about_open = true;
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("help_cat"), 3usize)); // AI 제어 페이지.
            }
            PaletteAction::OpenKnownHosts => self.known_hosts_open = true,
            PaletteAction::SaveOutput => self.save_focused_output(),
            PaletteAction::EditScrollback => self.edit_scrollback_in_pad(),
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
                // 메뉴와 동일하게 영속한다 — 팔레트로 켜면 재시작에 잊히던 드리프트(리뷰 2026-08-19).
                self.always_on_top = !self.always_on_top;
                self.pending_on_top = Some(self.always_on_top);
                self.config.appearance.always_on_top = self.always_on_top;
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
            PaletteAction::ZoomIn => self.set_font_size(self.font_size + 1.0),
            PaletteAction::ZoomOut => self.set_font_size(self.font_size - 1.0),
            PaletteAction::SetLang(l) => self.lang = l,
            PaletteAction::JumpDir(d) => self.spawn_local_at(d),
            PaletteAction::WorktreeCreate => self.open_worktree_prompt(),
            PaletteAction::WorktreeList => self.open_worktree_list(),
            PaletteAction::QuickSelect => self.quick_select_open = true,
            PaletteAction::RunHistory(cmd) => self.run_history_cmd(cmd),
            PaletteAction::PasteClip(t) => self.paste_text_to_focused(t),
            PaletteAction::FocusPane(p) => {
                if let Some(loc) = self.dock.find_tab(&p) {
                    let _ = self.dock.set_active_tab(loc);
                }
            }
            PaletteAction::DuplicateConnection => self.duplicate_connection(),
            PaletteAction::ToggleSessionLog => self.toggle_session_log(),
            PaletteAction::NewTabHere => self.spawn_here(),
            PaletteAction::ClearBuffer => self.clear_focused_scrollback(),
            PaletteAction::SyncUpload => self.sync_upload_diff(),
            PaletteAction::SyncDownload => self.sync_download_diff(),
            PaletteAction::CopyOutputMd => self.copy_last_output_md(ctx),
            PaletteAction::CompareFiles => self.compare_selected(),
            PaletteAction::FindDuplicates => self.find_duplicates(),
            PaletteAction::FindLargeFiles => self.find_large_files(),
            PaletteAction::CopySshCmd => self.copy_ssh_command(ctx),
            PaletteAction::GenSshKey => self.generate_ssh_key(ctx),
            PaletteAction::InstallPubkey => self.install_pubkey(),
            PaletteAction::SelToPad => self.selection_to_pad(),
            PaletteAction::CopyTabsMd => self.copy_open_tabs_md(ctx),
            PaletteAction::SaveAllDocs => {
                let n = self.save_all_docs();
                let msg = format!("{} {n}", tr(self.lang, "cmd.savedall"));
                self.notify = Some((msg, std::time::Instant::now()));
            }
            PaletteAction::OpenFileDialog => {
                if let Some(fp) = rfd::FileDialog::new().pick_file() {
                    self.open_editor_local(fp);
                }
            }
            PaletteAction::ScrollBottom => self.scroll_focused_bottom(),
            PaletteAction::ScrollTop => self.scroll_focused_top(),
            PaletteAction::ReplaceInFiles => self.replace_open = true,
            PaletteAction::DirTree => self.open_dir_tree(),
            PaletteAction::DirStats => self.open_dir_stats(),
            // 위 to_menu가 이미 메뉴 구현으로 보낸 항목들. 여기까지 왔다면 변환 표에서
            // 빠진 새 명령이라는 뜻이라 디버그 빌드에서 바로 드러나게 한다.
            other => debug_assert!(crate::palconv::to_menu(&other).is_some(), "팔레트 명령 미처리"),
        }
    }

    /// 히스토리에서 고른 명령을 포커스 pane에 개행과 함께 보낸다(재실행).
    pub(crate) fn run_history_cmd(&mut self, cmd: String) {
        let Some(pane) = self.focused_pane() else { return };
        let mut data = cmd.into_bytes();
        data.push(b'\r');
        self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
    }

    /// 포커스 pane의 스크롤백을 비운다.
    fn clear_focused_scrollback(&mut self) {
        let view = self
            .focused_pane()
            .and_then(|p| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()));
        if let Some(v) = view {
            if let Ok(mut md) = v.model.lock() {
                md.clear_scrollback();
            }
        }
    }
}
