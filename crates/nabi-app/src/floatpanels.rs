//! 분리 창(floating viewport) 본문 렌더 헬퍼 — 에디터/로컬 브라우저/SFTP. windows.rs에서 분리.

use crate::app::NabiApp;
use nabi_types::PaneId;

impl NabiApp {
    /// 분리 창의 내장 에디터 렌더 + 저장/다른이름저장/인코딩 처리.
    pub(crate) fn floating_editor(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let vctx = &ui.ctx().clone();
        let lang = self.lang;
        let recent = self.editor_config.recent_files.clone();
        let mut act = crate::editor::EditorAct::default();
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(e) = self.editors.get_mut(&pane) {
                act = crate::editortab::render_editor_tab(ui, e, lang, &recent);
            }
        });
        if act.toggle_menu_bar {
            self.toggle_editor_menu_bar();
        }
        if act.toggle_hex {
            self.toggle_editor_hex(pane);
        }
        if act.reload {
            self.reload_editor_doc(pane);
        }
        if act.complete_word {
            self.complete_word(pane);
        }
        if act.goto_last_edit {
            self.goto_last_edit(pane);
        }
        if act.save {
            self.save_editor_doc(pane);
        }
        if act.save_as {
            self.save_editor_as(pane);
        }
        if let Some(lbl) = act.set_encoding {
            self.reload_editor_encoding(pane, lbl);
        }
        if let Some(lbl) = act.save_encoding {
            self.save_with_encoding(pane, lbl);
        }
        if let Some(eol) = act.set_eol {
            self.convert_editor_eol(pane, eol);
        }
        if act.close {
            self.request_editor_close(pane); // #4: 미저장이면 확인 모달.
        }
        if act.open_settings {
            self.editor_settings_for = Some(pane);
        }
        if act.new_doc {
            self.open_empty_pad();
        }
        if act.open_file {
            if let Some(fp) = rfd::FileDialog::new().pick_file() {
                self.open_editor_local(fp);
            }
        }
        if let Some(rp) = act.open_recent {
            self.open_editor_local(rp.into());
        }
        if act.diff_disk {
            self.diff_editor_against_disk(pane);
        }
        if let Some(cmd) = act.run_in_term {
            self.run_in_first_terminal(cmd);
        }
        // LSP(T6-4 2단계): 분리 창에서도 동일 경로(드리프트 금지).
        if act.lsp_goto_def { self.lsp_goto_definition_for(pane); }
        if act.lsp_hover { self.lsp_hover_for(pane); }
        if act.lsp_refs { self.lsp_refs_for(pane); }
        if let Some(nm) = &act.lsp_rename { self.lsp_rename_for(pane, nm); }
        if act.lsp_format { self.lsp_format_for(pane); }
        if act.lsp_complete { self.lsp_complete_for(pane); }
        if let Some((path, line)) = act.open_at { self.open_editor_at(path, line); }
        // 분리 창이 자체 설정 창을 열었으면 이 창(vctx)에 렌더(단독 개발 대비 nabiPad 메뉴 자족).
        if self.editor_settings_for == Some(pane) {
            self.render_editor_settings(vctx);
        }
    }

    /// 분리 창의 로컬 파일 브라우저 렌더 + 액션 처리(central과 같은 스왑 적용 경로).
    pub(crate) fn floating_browser(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let vctx = &ui.ctx().clone();
        let remote_map = self.remote_compare_map();
        let can_upload = self.sftp.open && self.sftp.id.is_some();
        let lang = self.lang;
        let lrc = self.config.terminal.local_recent.clone();
        let mut act = None;
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(b) = self.browser_tabs.get_mut(&pane) {
                act = Some(crate::browser::render_browser_tab(ui, b, &remote_map, can_upload, lang, pane.get(), &lrc));
            }
        });
        if let Some(a) = act {
            if let Some(mut bp) = self.browser_tabs.remove(&pane) {
                std::mem::swap(&mut self.browser, &mut bp);
                self.apply_browser_act(vctx, a);
                std::mem::swap(&mut self.browser, &mut bp);
                self.browser_tabs.insert(pane, bp);
            }
        }
    }

    /// 분리 창의 SFTP 파일브라우저 렌더 + 액션 처리(활성 패널만 액션 처리, 배경은 표시).
    pub(crate) fn floating_sftp(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let vctx = &ui.ctx().clone();
        let lang = self.lang;
        let active = Some(pane) == self.sftp_pane;
        let bm = self.config.terminal.sftp_bookmarks.clone();
        let rc = self.config.terminal.sftp_recent.clone();
        let sd = (self.browser.sort, self.browser.sort_desc); // 클로저가 self를 가변 차용하므로 미리 캡처.
        let mut act = crate::sftptab::SftpAct::default();
        egui::CentralPanel::default().show(ui, |ui| {
            let panel = if active {
                &mut self.sftp
            } else if let Some(p) = self.sftp_bg.get_mut(&pane) {
                p
            } else {
                return;
            };
            act = crate::sftptab::render_sftp_tab(ui, panel, lang, &bm, &rc, sd);
        });
        if active {
            self.process_sftp_act(act, vctx);
        }
    }
}
