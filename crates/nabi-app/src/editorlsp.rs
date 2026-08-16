//! nabiPad LSP 허브(T6-4 1단계) — rust-analyzer 진단·정의 이동을 앱에 연결.
//!
//! v1 범위: 로컬 `.rs` 텍스트 문서만, 서버는 rust-analyzer 하나(첫 rs 문서의
//! Cargo.toml 루트에서 지연 기동). 서버가 없으면 조용히 비활성 — 에디터는 평소대로.

use crate::app::NabiApp;
use nabi_editor::lspclient::LspClient;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// didChange 디바운스 — 타자 중 매 프레임 전송을 막는다.
const DEBOUNCE_MS: u128 = 400;

#[derive(Default)]
pub struct LspHub {
    pub client: Option<LspClient>,
    /// 기동 실패(서버 없음 등) — 세션 내 재시도하지 않는다.
    failed: bool,
    /// 문서별 마지막 동기화 텍스트 해시.
    synced: HashMap<PathBuf, u64>,
    /// 변경 감지 시각(디바운스 기준). 해시가 다시 바뀌면 갱신.
    changed_at: HashMap<PathBuf, (u64, Instant)>,
    /// 대기 중인 정의 이동 요청 id.
    pub(crate) pending_def: Option<i64>,
    /// 대기 중인 심볼 정보/참조 요청: (요청 id, 대상 pane).
    pub(crate) pending_hover: Option<(i64, nabi_types::PaneId)>,
    pub(crate) pending_refs: Option<(i64, nabi_types::PaneId)>,
    pub(crate) pending_rename: Option<i64>,
    pub(crate) pending_fmt: Option<(i64, nabi_types::PaneId)>,
    /// 자동완성: (요청 id, pane, 앵커 문자 오프셋) + 자동 트리거 중복 방지(오프셋, 해시).
    pub(crate) pending_comp: Option<(i64, nabi_types::PaneId, usize)>,
    pub(crate) comp_last: HashMap<nabi_types::PaneId, (usize, u64)>,
}

/// 텍스트 해시(FNV-1a) — 프레임당 rs 문서 몇 개 수준이라 충분히 싸다.
fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 문자 오프셋 → LSP 위치(0기반 줄, UTF-16 열).
fn lsp_pos(text: &str, off: usize) -> (u32, u32) {
    let (mut line, mut col) = (0u32, 0u32);
    for (i, ch) in text.chars().enumerate() {
        if i >= off {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    (line, col)
}

/// path에서 위로 올라가며 Cargo.toml이 있는 프로젝트 루트를 찾는다.
fn cargo_root(path: &Path) -> Option<PathBuf> {
    let mut dir = path.parent()?;
    loop {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// 이 문서가 LSP 대상인가(로컬 rs 텍스트 문서 — HEX/대용량/원격 제외).
fn lsp_doc(doc: &nabi_editor::editor::EditorDoc) -> bool {
    doc.loaded
        && doc.remote.is_none()
        && doc.hex.is_none()
        && doc.big.is_none()
        && doc.edit.is_none()
        && doc.lang_ext() == "rs"
        && doc.path.is_absolute()
}

impl NabiApp {
    /// 매 프레임: rs 문서 동기화(didOpen/didChange 디바운스) + 진단 수신 + 정의 응답 처리.
    pub(crate) fn lsp_tick(&mut self) {
        let ids: Vec<nabi_types::PaneId> = self.editors.keys().copied().collect();
        for id in ids {
            let Some(doc) = self.editors.get(&id) else { continue };
            if !lsp_doc(doc) {
                continue;
            }
            let (path, hash) = (doc.path.clone(), fnv(&doc.text));
            // 지연 기동: 첫 rs 문서의 Cargo.toml 루트에서 rust-analyzer 시작.
            if self.lsp.client.is_none() && !self.lsp.failed {
                let Some(root) = cargo_root(&path) else { continue };
                self.lsp.client = LspClient::start("rust-analyzer", &root);
                self.lsp.failed = self.lsp.client.is_none();
            }
            let Some(c) = &self.lsp.client else { continue };
            if !c.ready() {
                continue;
            }
            match self.lsp.synced.get(&path) {
                None => {
                    let doc = &self.editors[&id];
                    c.did_open(&path, &doc.text);
                    self.lsp.synced.insert(path.clone(), hash);
                }
                Some(prev) if *prev != hash => {
                    // 변경 감지 → 디바운스 후 전체 텍스트 재동기화.
                    let e = self.lsp.changed_at.entry(path.clone()).or_insert((hash, Instant::now()));
                    if e.0 != hash {
                        *e = (hash, Instant::now());
                    } else if e.1.elapsed().as_millis() >= DEBOUNCE_MS {
                        let doc = &self.editors[&id];
                        c.did_change(&path, &doc.text);
                        self.lsp.synced.insert(path.clone(), hash);
                        self.lsp.changed_at.remove(&path);
                    }
                }
                _ => {}
            }
            // 진단을 문서에 반영(거터 점·상태바가 그린다) + 서버 상태 표시.
            let (diags, st) = (c.diagnostics(&path), if c.ready() { 2 } else { 1 });
            if let Some(doc) = self.editors.get_mut(&id) {
                doc.diags = diags.into_iter().map(|d| (d.line as usize, d.severity, d.message)).collect();
                doc.lsp_state = st;
            }
        }
        // 자동완성 자동 트리거: 방금 '.' 또는 '::'를 타이핑한 순간(중복 방지: 같은 오프셋+해시).
        let mut want_comp = None;
        for (id, doc) in &self.editors {
            if !lsp_doc(doc) || doc.lsp_comp.is_some() {
                continue;
            }
            let (off, h) = (doc.cur_off, fnv(&doc.text));
            if self.lsp.comp_last.get(id) == Some(&(off, h)) {
                continue;
            }
            let tail: Vec<char> = doc.text.chars().take(off).collect::<Vec<_>>().iter().rev().take(2).copied().collect();
            let trig = matches!(tail.first(), Some('.')) || (tail.len() == 2 && tail[0] == ':' && tail[1] == ':');
            if trig {
                want_comp = Some((*id, off, h));
                break;
            }
        }
        if let Some((id, off, h)) = want_comp {
            self.lsp.comp_last.insert(id, (off, h));
            self.lsp_complete_for(id);
        }
        // 자동완성 응답 폴링 — 후보를 문서 팝업 상태에 넣는다(빈 목록=조용히 무시).
        if let (Some((rid, pane, anchor)), Some(c)) = (self.lsp.pending_comp, &self.lsp.client) {
            if let Some(items) = c.take_completion(rid) {
                self.lsp.pending_comp = None;
                if let (false, Some(doc)) = (items.is_empty(), self.editors.get_mut(&pane)) {
                    doc.lsp_comp = Some(items);
                    doc.comp_anchor = anchor;
                }
            }
        }
        // 심볼 정보/참조 응답 폴링 — 도착하면 해당 문서 팝업 상태에 넣는다(editorcode가 그림).
        if let (Some((id, pane)), Some(c)) = (self.lsp.pending_hover, &self.lsp.client) {
            if let Some(reply) = c.take_hover(id) {
                self.lsp.pending_hover = None;
                match (reply, self.editors.get_mut(&pane)) {
                    (Some(text), Some(doc)) => doc.lsp_info = Some(text),
                    (None, _) => self.notify = Some((nabi_i18n::tr(self.lang, "lsp.noinfo").to_string(), Instant::now())),
                    _ => {}
                }
            }
        }
        if let (Some((id, pane)), Some(c)) = (self.lsp.pending_refs, &self.lsp.client) {
            if let Some(locs) = c.take_references(id) {
                self.lsp.pending_refs = None;
                if locs.is_empty() {
                    self.notify = Some((nabi_i18n::tr(self.lang, "lsp.norefs").to_string(), Instant::now()));
                } else if let Some(doc) = self.editors.get_mut(&pane) {
                    doc.lsp_refs = Some(locs.into_iter().map(|l| (l.path.to_string_lossy().into_owned(), l.line, l.col)).collect());
                }
            }
        }
        // 포맷팅 응답 폴링 — 전체 문서 TextEdit를 메모리에 적용(저장은 사용자 몫).
        if let (Some((id, pane)), Some(c)) = (self.lsp.pending_fmt, &self.lsp.client) {
            if let Some(edits) = c.take_formatting(id) {
                self.lsp.pending_fmt = None;
                if let Some(doc) = self.editors.get_mut(&pane) {
                    if edits.is_empty() {
                        self.notify = Some((nabi_i18n::tr(self.lang, "lsp.fmt.clean").to_string(), Instant::now()));
                    } else {
                        doc.text = nabi_editor::lspread::apply_edits(&doc.text, &edits);
                        doc.dirty = true;
                        self.notify = Some((nabi_i18n::tr(self.lang, "lsp.fmt.done").to_string(), Instant::now()));
                    }
                }
            }
        }
        // 이름 바꾸기 응답 폴링 — WorkspaceEdit를 열린 문서(메모리)와 디스크에 적용.
        if let (Some(id), Some(c)) = (self.lsp.pending_rename, &self.lsp.client) {
            if let Some(files) = c.take_rename(id) {
                self.lsp.pending_rename = None;
                if files.is_empty() {
                    self.notify = Some((nabi_i18n::tr(self.lang, "lsp.norename").to_string(), Instant::now()));
                } else {
                    let n = self.apply_rename_edits(files);
                    self.notify = Some((format!("{} {n}", nabi_i18n::tr(self.lang, "lsp.renamed")), Instant::now()));
                }
            }
        }
        // 정의 이동 응답 폴링 — 도착하면 해당 파일을 열어 그 줄로 점프.
        if let (Some(reqid), Some(c)) = (self.lsp.pending_def, &self.lsp.client) {
            if let Some(reply) = c.take_definition(reqid) {
                self.lsp.pending_def = None;
                match reply {
                    Some(def) => self.open_editor_at(def.path.to_string_lossy().into_owned(), def.line as usize),
                    None => self.notify = Some((nabi_i18n::tr(self.lang, "lsp.nodef").to_string(), Instant::now())),
                }
            }
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

    /// 지정 pane의 rs 문서에서 커서 위치 LSP 요청을 보낼 준비(공용 게이트).
    pub(crate) fn lsp_req_ctx(&mut self, p: nabi_types::PaneId) -> Option<(PathBuf, u32, u32)> {
        let doc = self.editors.get(&p)?;
        if !lsp_doc(doc) {
            return None;
        }
        if self.lsp.client.is_none() {
            self.notify = Some((nabi_i18n::tr(self.lang, "lsp.off").to_string(), Instant::now()));
            return None;
        }
        let (line, col) = lsp_pos(&doc.text, doc.cur_off);
        Some((doc.path.clone(), line, col))
    }

    /// 지정 pane에서 정의로 이동 요청(컨텍스트/메뉴 경유).
    pub(crate) fn lsp_goto_definition_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            self.lsp.pending_def = self.lsp.client.as_ref().and_then(|c| c.request_definition(&path, line, col));
        }
    }

    /// 지정 pane에서 심볼 정보(hover) 요청.
    pub(crate) fn lsp_hover_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            self.lsp.pending_hover = self.lsp.client.as_ref().and_then(|c| c.request_hover(&path, line, col)).map(|id| (id, p));
        }
    }

    /// 지정 pane에서 참조 찾기 요청.
    pub(crate) fn lsp_refs_for(&mut self, p: nabi_types::PaneId) {
        if let Some((path, line, col)) = self.lsp_req_ctx(p) {
            self.lsp.pending_refs = self.lsp.client.as_ref().and_then(|c| c.request_references(&path, line, col)).map(|id| (id, p));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_pos_counts_utf16_columns() {
        assert_eq!(lsp_pos("ab\ncd", 4), (1, 1), "둘째 줄 두 번째 문자");
        // '한'은 UTF-16 1유닛, '𐍈'(U+10348)은 2유닛.
        let t = "한\u{10348}x";
        assert_eq!(lsp_pos(t, 2), (0, 3), "서러게이트 쌍은 2열 차지");
    }

    #[test]
    fn cargo_root_walks_up() {
        let d = std::env::temp_dir().join(format!("nabi-lsproot-{}", std::process::id()));
        let deep = d.join("src").join("sub");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(d.join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(cargo_root(&deep.join("a.rs")), Some(d.clone()));
        let _ = std::fs::remove_dir_all(&d);
    }
}
