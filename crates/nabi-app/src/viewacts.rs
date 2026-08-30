//! 도크 렌더가 모아 온 탭 액션을 적용한다(브라우저 탭·에디터 탭·탭 우클릭 SFTP 열기).
//!
//! 렌더 중에는 `self`의 여러 부분을 동시에 빌리고 있어 여기서 처리할 수 없다.
//! 그래서 렌더는 액션만 모으고, 빌림이 끝난 뒤 이 함수들이 실제 상태를 바꾼다(view.rs).

use crate::app::NabiApp;
use crate::browser::BrowserAct;
use crate::editor::EditorAct;
use nabi_types::PaneId;

impl NabiApp {
    /// 탭 우클릭 'SFTP 열기' — 그 SSH pane의 출처(host/user/자격증명)로 원격 브라우저를 연다.
    pub(crate) fn open_sftp_from_pane(&mut self, p: PaneId) {
        let Some(nabi_session::SessionKind::Ssh { host, port, user, credential_ref, key_path, jump, agent_forward }) =
            self.pane_origins.get(&p).cloned()
        else {
            return;
        };
        let name = format!("{user}@{host}");
        let kind = nabi_session::SessionKind::Ssh { host, port, user, credential_ref, key_path, jump, agent_forward };
        let session = nabi_session::SavedSession {
            name,
            folder: None,
            kind,
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        };
        self.open_sftp_saved(session, false);
    }

    /// 브라우저 탭 액션 적용(동시에 보이는 모든 탭).
    ///
    /// 사이드패널 상태와 잠시 스왑해 기존 적용 경로를 그대로 쓴다 — 전송 목적지 등이
    /// `self.browser.path`를 읽으므로 스왑이 곧 정확한 컨텍스트가 된다.
    pub(crate) fn apply_browser_tab_acts(
        &mut self,
        ctx: &egui::Context,
        acts: Vec<(PaneId, BrowserAct)>,
        closed: Option<PaneId>,
    ) {
        for (p, a) in acts {
            if let Some(r) = a.rect {
                self.drop_zones.push((crate::dnd::DropTarget::BrowserTab(p), r));
            }
            if let Some(mut bp) = self.browser_tabs.remove(&p) {
                std::mem::swap(&mut self.browser, &mut bp);
                self.apply_browser_act(ctx, a);
                std::mem::swap(&mut self.browser, &mut bp);
                self.browser_tabs.insert(p, bp);
            }
        }
        if let Some(p) = closed {
            self.browser_tabs.remove(&p); // 닫힌 탭 상태 폐기.
        }
    }

    /// 에디터 탭 액션(저장·인코딩·닫기 등) 적용 + 닫힘 정리.
    pub(crate) fn apply_editor_tab_acts(&mut self, acts: Vec<(PaneId, EditorAct)>, closed: Option<PaneId>) {
        for (p, a) in acts {
            // 편집기가 혼자 파일을 쓰다 실패한 것 — 알릴 화면이 앱에만 있다.
            if let Some(e) = &a.failed {
                let msg = format!("{} — {e}", nabi_i18n::tr(self.lang, "editor.writefailed"));
                self.notify = Some((msg, std::time::Instant::now()));
            }
            if a.toggle_menu_bar { self.toggle_editor_menu_bar(); }
            if a.toggle_hex { self.toggle_editor_hex(p); }
            if a.reload { self.reload_editor_doc(p); }
            if a.goto_last_edit { self.goto_last_edit(p); }
            if a.complete_word { self.complete_word(p); }
            if let Some(s) = a.diff_restore { self.restore_diff_side(p, s); }
            if a.find_secrets { self.find_secrets_in_doc(p); }
            if a.save { self.save_editor_doc(p); }
            if a.save_all {
                let n = self.save_all_docs();
                let msg = format!("{} {n}", nabi_i18n::tr(self.lang, "cmd.savedall"));
                self.notify = Some((msg, std::time::Instant::now()));
            }
            if a.save_as { self.save_editor_as(p); }
            if let Some(lbl) = a.set_encoding { self.reload_editor_encoding(p, lbl); }
            if let Some(lbl) = a.save_encoding { self.save_with_encoding(p, lbl); }
            if let Some(eol) = a.set_eol { self.convert_editor_eol(p, eol); }
            if a.close { self.request_editor_close(p); } // 미저장이면 확인 모달.
            if a.open_settings { self.editor_settings_for = Some(p); }
            if a.new_doc { self.open_empty_pad(); }
            if a.open_file {
                if let Some(fp) = rfd::FileDialog::new().pick_file() {
                    self.open_editor_local(fp);
                }
            }
            if let Some(rp) = a.open_recent { self.open_editor_local(rp.into()); }
            if a.diff_disk { self.diff_editor_against_disk(p); }
            if a.diff_open { self.open_compare_picker(p); }
            if let Some(c) = a.run_in_term { self.run_in_first_terminal(c); }
            // LSP(T6-4 2단계): 컨텍스트/메뉴에서 온 코드 요청 + 참조 목록의 타 파일 점프.
            if a.lsp_goto_def { self.lsp_goto_definition_for(p); }
            if a.lsp_hover { self.lsp_hover_for(p); }
            if a.lsp_refs { self.lsp_refs_for(p); }
            if let Some(nm) = &a.lsp_rename { self.lsp_rename_for(p, nm); }
            if a.lsp_format { self.lsp_format_for(p); }
            if a.lsp_complete { self.lsp_complete_for(p); }
            if let Some((path, line)) = a.open_at { self.open_editor_at(path, line); }
        }
        if let Some(p) = closed {
            self.editors.remove(&p);
        }
    }
}
