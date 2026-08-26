//! LSP **요청 보내기**(정의·심볼 정보·참조) — 응답 폴링은 `editorlsp`, 나머지는 `editorlsp2`.
//!
//! 본체가 소프트 한도에 닿아 나눴다. 요청은 셋 다 같은 모양이다: 커서 자리를 구하고,
//! 그 문서를 맡은 서버에 보내고, **어느 서버에 보냈는지**(키)를 함께 기억한다.
//! 키를 함께 두는 까닭은 서버마다 요청 번호를 1부터 세기 때문이다.

use crate::app::NabiApp;
use crate::editorlsp::{lsp_doc, lsp_pos};
use std::path::PathBuf;
use std::time::Instant;

impl NabiApp {
    /// 지정 pane의 rs 문서에서 커서 위치 LSP 요청을 보낼 준비(공용 게이트).
    pub(crate) fn lsp_req_ctx(&mut self, p: nabi_types::PaneId) -> Option<(PathBuf, u32, u32)> {
        let doc = self.editors.get(&p)?;
        if !lsp_doc(doc) {
            return None;
        }
        // 이 **문서를 맡은** 서버가 있어야 한다(다른 언어 서버가 떠 있어도 소용없다).
        if self.lsp.client_for(&doc.path).is_none() {
            self.notify = Some((nabi_i18n::tr(self.lang, "lsp.off").to_string(), Instant::now()));
            return None;
        }
        let (line, col) = lsp_pos(&doc.text, doc.cur_off);
        Some((doc.path.clone(), line, col))
    }

    /// 지정 pane에서 정의로 이동 요청(컨텍스트/메뉴 경유).
    pub(crate) fn lsp_goto_definition_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            let key = self.lsp.key_for(&path);
            self.lsp.pending_def = self.lsp.client_for(&path).and_then(|c| c.request_definition(&path, line, col)).zip(key);
        }
    }

    /// 지정 pane에서 심볼 정보(hover) 요청.
    pub(crate) fn lsp_hover_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            let key = self.lsp.key_for(&path);
            self.lsp.pending_hover = self.lsp.client_for(&path).and_then(|c| c.request_hover(&path, line, col)).zip(key).map(|(id, k)| (id, p, k));
        }
    }

    /// 지정 pane에서 참조 찾기 요청.
    pub(crate) fn lsp_refs_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            let key = self.lsp.key_for(&path);
            self.lsp.pending_refs = self.lsp.client_for(&path).and_then(|c| c.request_references(&path, line, col)).zip(key).map(|(id, k)| (id, p, k));
        }
    }

    /// 파일을 열고 지정 줄(0기반)로 점프(참조 목록 등 위치 점프 공용).
    pub(crate) fn open_editor_at(&mut self, path: String, line: usize) {
        let pb = PathBuf::from(&path);
        self.open_editor_local(pb.clone());
        if let Some(d) = self.editors.values_mut().find(|d| d.path == pb) {
            d.jump_to_line(line);
        }
    }

    /// 못 띄운 언어 서버를 **한 번씩만** 알린다.
    ///
    /// 매 프레임 같은 말을 띄우면 알림이 아니라 방해가 된다. 기억해 뒀다가 처음 한 번만.
    pub(crate) fn announce_missing_lsp(&mut self) {
        let cmds: Vec<&'static str> = self.lsp.missing.drain().collect();
        for cmd in cmds {
            let label = nabi_editor::lspservers::SERVERS
                .iter()
                .find(|s| s.cmd == cmd)
                .map(|s| s.label)
                .unwrap_or(cmd);
            let msg = format!("{} {label}: {cmd}", nabi_i18n::tr(self.lang, "lsp.missing"));
            self.notify = Some((msg, Instant::now()));
        }
    }
}
