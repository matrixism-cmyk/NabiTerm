//! nabiPad LSP 요청 진입점(T6-4) — 포맷팅·이름 바꾸기·팔레트 래퍼. editorlsp.rs에서 분리(라인 한도).

use crate::app::NabiApp;

impl NabiApp {
    /// 지정 pane에서 문서 전체 포맷팅 요청(rustfmt).
    pub(crate) fn lsp_format_for(&mut self, p: nabi_types::PaneId) {
        if self.lsp_req_ctx(p).is_some() {
            let doc = &self.editors[&p];
            let (path, tab) = (doc.path.clone(), self.editor_config.tab_size.max(1) as u32);
            let key = self.lsp.key_for(&path);
            self.lsp.pending_fmt = self.lsp.client_for(&path)
                .and_then(|c| c.request_formatting(&path, tab))
                .zip(key)
                .map(|(id, k)| (id, p, k));
        }
    }

    /// 지정 pane에서 자동완성 요청 — 앵커는 커서 앞 식별자 시작(접두어 필터·대치 기준).
    pub(crate) fn lsp_complete_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            let doc = &self.editors[&p];
            let chars: Vec<char> = doc.text.chars().collect();
            let mut a = doc.cur_off.min(chars.len());
            while a > 0 && (chars[a - 1].is_alphanumeric() || chars[a - 1] == '_') {
                a -= 1;
            }
            // 요청 전 즉시 동기화(디바운스 우회) — 서버가 최신 텍스트 기준으로 답한다.
            if let Some(c) = self.lsp.client_for(&path) {
                c.did_change(&path, &doc.text);
            }
            let key = self.lsp.key_for(&path);
            self.lsp.pending_comp = self.lsp.client_for(&path)
                .and_then(|c| c.request_completion(&path, line, col))
                .zip(key)
                .map(|(id, k)| (id, p, a, k));
        }
    }

    /// 지정 pane에서 심볼 이름 바꾸기 요청(입력 팝업 확정 후).
    pub(crate) fn lsp_rename_for(&mut self, p: nabi_types::PaneId, new_name: &str) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            let key = self.lsp.key_for(&path);
            self.lsp.pending_rename = self.lsp.client_for(&path)
                .and_then(|c| c.request_rename(&path, line, col, new_name))
                .zip(key);
        }
    }

    /// WorkspaceEdit 적용: 열린 문서는 메모리에서(수정 표시), 닫힌 파일은 디스크에서. 총 편집 수 반환.
    pub(crate) fn apply_rename_edits(&mut self, files: Vec<nabi_editor::lspread::FileEdits>) -> usize {
        let mut n = 0;
        for fe in files {
            n += fe.edits.len();
            if let Some(doc) = self.editors.values_mut().find(|d| d.path == fe.path && d.edit.is_none() && d.hex.is_none()) {
                doc.text = nabi_editor::lspread::apply_edits(&doc.text, &fe.edits);
                doc.dirty = true;
            } else if let Ok(text) = std::fs::read_to_string(&fe.path) {
                // LF 정규화 없이 그대로 적용 — LSP 좌표는 서버가 준 원문 기준.
                let _ = std::fs::write(&fe.path, nabi_editor::lspread::apply_edits(&text, &fe.edits));
            } else {
                n -= fe.edits.len(); // 읽기 실패 파일은 계수 제외.
            }
        }
        n
    }

    /// 팔레트 "정의로 이동": 포커스된 rs 문서 기준.
    pub(crate) fn lsp_goto_definition(&mut self) {
        if let Some(p) = self.focused_pane() {
            self.lsp_goto_definition_for(p);
        }
    }

    /// 팔레트 "심볼 정보"/"참조 찾기": 포커스된 rs 문서 기준.
    pub(crate) fn lsp_hover(&mut self) {
        if let Some(p) = self.focused_pane() {
            self.lsp_hover_for(p);
        }
    }
    pub(crate) fn lsp_refs(&mut self) {
        if let Some(p) = self.focused_pane() {
            self.lsp_refs_for(p);
        }
    }
}
