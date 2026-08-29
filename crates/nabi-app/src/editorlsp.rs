//! nabiPad LSP 허브(T6-4 1단계) — rust-analyzer 진단·정의 이동을 앱에 연결.
//!
//! v1 범위: 로컬 `.rs` 텍스트 문서만, 서버는 rust-analyzer 하나(첫 rs 문서의
//! Cargo.toml 루트에서 지연 기동). 서버가 없으면 조용히 비활성 — 에디터는 평소대로.

use crate::app::NabiApp;
use nabi_editor::lspclient::LspClient;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// didChange 디바운스 — 타자 중 매 프레임 전송을 막는다.
const DEBOUNCE_MS: u128 = 400;

/// 언어 서버 여러 개를 함께 관리한다.
///
/// 예전에는 `client` 하나에 rust-analyzer만 있었다. 문서마다 언어가 다르므로 서버도
/// 여럿이어야 하는데, 그러면 **요청 번호가 겹친다** — 두 서버가 각자 1번부터 센다.
/// 그래서 대기 중인 요청마다 **어느 서버의 것인지**(키)를 함께 들고 다닌다.
#[derive(Default)]
pub struct LspHub {
    /// 키(`명령\0루트`) → 서버. 같은 언어·같은 프로젝트면 하나를 공유한다.
    clients: HashMap<String, LspClient>,
    /// 문서 → 그 문서를 맡은 서버 키.
    doc_key: HashMap<PathBuf, String>,
    /// 기동에 실패한 서버들 — **세션 내 재시도하지 않는다**(매 프레임 다시 띄우면 안 된다).
    failed: std::collections::HashSet<String>,
    /// 서버가 없어 못 붙은 언어(안내를 한 번만 띄우려고 기억한다).
    pub(crate) missing: std::collections::HashSet<&'static str>,
    /// 문서별 마지막 동기화 텍스트 해시.
    synced: HashMap<PathBuf, u64>,
    /// 변경 감지 시각(디바운스 기준). 해시가 다시 바뀌면 갱신.
    changed_at: HashMap<PathBuf, (u64, Instant)>,
    /// 대기 중인 정의 이동 요청 id.
    /// (요청 번호, **어느 서버**). 번호만 들면 다른 서버의 응답을 제 것으로 착각한다.
    pub(crate) pending_def: Option<(i64, String)>,
    /// 대기 중인 심볼 정보/참조 요청: (요청 id, 대상 pane).
    pub(crate) pending_hover: Option<(i64, nabi_types::PaneId, String)>,
    pub(crate) pending_refs: Option<(i64, nabi_types::PaneId, String)>,
    pub(crate) pending_rename: Option<(i64, String)>,
    pub(crate) pending_fmt: Option<(i64, nabi_types::PaneId, String)>,
    /// 자동완성: (요청 id, pane, 앵커 문자 오프셋) + 자동 트리거 중복 방지(오프셋, 해시).
    pub(crate) pending_comp: Option<(i64, nabi_types::PaneId, usize, String)>,
    pub(crate) comp_last: HashMap<nabi_types::PaneId, (usize, u64)>,
}

impl LspHub {
    /// 이 문서를 맡은 서버(없으면 None).
    pub(crate) fn client_for(&self, path: &std::path::Path) -> Option<&LspClient> {
        self.clients.get(self.doc_key.get(path)?)
    }

    /// 이 문서를 맡은 서버의 키(요청과 함께 들고 다닌다).
    pub(crate) fn key_for(&self, path: &std::path::Path) -> Option<String> {
        self.doc_key.get(path).cloned()
    }

    /// 그 키의 서버(대기 중인 응답을 꺼낼 때 쓴다).
    pub(crate) fn by_key(&self, key: &str) -> Option<&LspClient> {
        self.clients.get(key)
    }

    /// 이 문서를 맡을 서버를 띄우거나 이미 있으면 그대로 쓴다. 키를 돌려준다.
    ///
    /// 없는 서버는 **한 번만** 시도한다 — 매 프레임 프로세스를 띄우려 들면 안 된다.
    pub(crate) fn ensure_for(&mut self, path: &std::path::Path) -> Option<String> {
        if let Some(k) = self.doc_key.get(path) {
            return Some(k.clone());
        }
        let ext = path.extension()?.to_string_lossy().into_owned();
        let srv = nabi_editor::lspservers::for_ext(&ext)?;
        let root = nabi_editor::lspservers::project_root(path, srv.markers)?;
        let key = format!("{}\0{}", srv.cmd, root.display());
        if self.failed.contains(&key) {
            return None;
        }
        if !self.clients.contains_key(&key) {
            match LspClient::start_with(srv.cmd, srv.args, &root) {
                Some(c) => {
                    self.clients.insert(key.clone(), c);
                }
                None => {
                    self.failed.insert(key);
                    self.missing.insert(srv.cmd); // 화면에 한 번 알리기 위해.
                    return None;
                }
            }
        }
        self.doc_key.insert(path.to_path_buf(), key.clone());
        Some(key)
    }
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
pub(crate) fn lsp_pos(text: &str, off: usize) -> (u32, u32) {
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


/// 이 문서가 LSP 대상인가(로컬 텍스트 문서 — HEX/대용량/원격 제외).
///
/// 예전에는 `== "rs"`가 박혀 있었다. 이제 **표가 아는 언어면** 대상이다. 원격 파일은
/// 여전히 제외한다 — 언어 서버는 로컬 파일 경로로만 일한다.
pub(crate) fn lsp_doc(doc: &nabi_editor::editor::EditorDoc) -> bool {
    doc.loaded
        && doc.remote.is_none()
        && doc.hex.is_none()
        && doc.big.is_none()
        && doc.edit.is_none()
        && nabi_editor::lspservers::for_ext(&doc.lang_ext()).is_some()
        && doc.path.is_absolute()
}

impl NabiApp {
    /// 저장했다고 언어 서버에 알린다.
    ///
    /// ## 왜 따로 적어 두는가
    ///
    /// `didOpen`·`didChange` 는 보내고 있었는데 **`didSave` 만 빠져 있었다**(2026-08-30
    /// 전수 점검에서 찾았다 — `did_save` 를 아무도 부르지 않았다). rust-analyzer 는 저장
    /// 통지를 받아야 `cargo check` 를 돌린다. 그래서 저장해도 진짜 진단이 갱신되지 않고
    /// 파일 안만 보고 아는 것들만 나왔다. 편집기를 가늠하는 자리에서 가장 눈에 띄는 결함이다.
    ///
    /// 보내기 직전에 글도 한 번 맞춘다 — 디바운스 때문에 서버가 옛 글을 들고 있으면
    /// 진단이 엉뚱한 줄을 가리킨다.
    pub(crate) fn lsp_did_save(&mut self, path: &std::path::Path) {
        let text = self.editors.values().find(|d| d.path == path).map(|d| d.text.clone());
        if let Some(c) = self.lsp.client_for(path) {
            if let Some(t) = text {
                c.did_change(path, &t);
            }
            c.did_save(path);
        }
    }
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
            // 지연 기동: 그 언어의 서버를 그 프로젝트 루트에서 띄운다(이미 있으면 그대로).
            if self.lsp.ensure_for(&path).is_none() {
                // 서버가 없으면 **왜 없는지** 한 번 알린다. 조용히 비활성이면 사용자는
                // 고장으로 읽는다 — "pyright를 깔면 됩니다"는 알려 줄 수 있는 정보다.
                self.announce_missing_lsp();
                continue; // 편집기는 평소대로 동작한다.
            }
            // 서버 손잡이를 **오래 들고 있지 않는다** — 들고 있으면 `self.lsp`의 다른 칸
            // (동기화 기록)을 못 고친다. 그래서 매번 키로 다시 찾아 짧게 쓴다.
            let Some(key) = self.lsp.key_for(&path) else { continue };
            if !self.lsp.by_key(&key).is_some_and(|c| c.ready()) {
                continue;
            }
            match self.lsp.synced.get(&path).copied() {
                None => {
                    let text = self.editors[&id].text.clone();
                    if let Some(c) = self.lsp.by_key(&key) {
                        c.did_open(&path, &text);
                    }
                    self.lsp.synced.insert(path.clone(), hash);
                }
                Some(prev) if prev != hash => {
                    // 변경 감지 → 디바운스 후 전체 텍스트 재동기화.
                    let e = self.lsp.changed_at.entry(path.clone()).or_insert((hash, Instant::now()));
                    let due = match e.0 != hash {
                        true => {
                            *e = (hash, Instant::now());
                            false
                        }
                        false => e.1.elapsed().as_millis() >= DEBOUNCE_MS,
                    };
                    if due {
                        let text = self.editors[&id].text.clone();
                        if let Some(c) = self.lsp.by_key(&key) {
                            c.did_change(&path, &text);
                        }
                        self.lsp.synced.insert(path.clone(), hash);
                        self.lsp.changed_at.remove(&path);
                    }
                }
                _ => {}
            }
            // 진단을 문서에 반영(거터 점·상태바가 그린다) + 서버 상태 표시.
            let Some(c) = self.lsp.by_key(&key) else { continue };
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
        if let Some((rid, pane, anchor, c)) = self.lsp.pending_comp.clone().and_then(|(r, p, a, k)| self.lsp.by_key(&k).map(|c| (r, p, a, c))) {
            if let Some(items) = c.take_completion(rid) {
                self.lsp.pending_comp = None;
                if let (false, Some(doc)) = (items.is_empty(), self.editors.get_mut(&pane)) {
                    doc.lsp_comp = Some(items);
                    doc.comp_anchor = anchor;
                }
            }
        }
        // 심볼 정보/참조 응답 폴링 — 도착하면 해당 문서 팝업 상태에 넣는다(editorcode가 그림).
        if let Some((id, pane, c)) = self.lsp.pending_hover.clone().and_then(|(i, p, k)| self.lsp.by_key(&k).map(|c| (i, p, c))) {
            if let Some(reply) = c.take_hover(id) {
                self.lsp.pending_hover = None;
                match (reply, self.editors.get_mut(&pane)) {
                    (Some(text), Some(doc)) => doc.lsp_info = Some(text),
                    (None, _) => self.notify = Some((nabi_i18n::tr(self.lang, "lsp.noinfo").to_string(), Instant::now())),
                    _ => {}
                }
            }
        }
        if let Some((id, pane, c)) = self.lsp.pending_refs.clone().and_then(|(i, p, k)| self.lsp.by_key(&k).map(|c| (i, p, c))) {
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
        if let Some((id, pane, c)) = self.lsp.pending_fmt.clone().and_then(|(i, p, k)| self.lsp.by_key(&k).map(|c| (i, p, c))) {
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
        if let Some((id, c)) = self.lsp.pending_rename.clone().and_then(|(i, k)| self.lsp.by_key(&k).map(|c| (i, c))) {
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
        if let Some((reqid, c)) = self.lsp.pending_def.clone().and_then(|(i, k)| self.lsp.by_key(&k).map(|c| (i, c))) {
            if let Some(reply) = c.take_definition(reqid) {
                self.lsp.pending_def = None;
                match reply {
                    Some(def) => self.open_editor_at(def.path.to_string_lossy().into_owned(), def.line as usize),
                    None => self.notify = Some((nabi_i18n::tr(self.lang, "lsp.nodef").to_string(), Instant::now())),
                }
            }
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

}