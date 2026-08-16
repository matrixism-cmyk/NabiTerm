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
    pending_def: Option<i64>,
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
            // 진단을 문서에 반영(거터 점·상태바가 그린다).
            let diags = c.diagnostics(&path);
            if let Some(doc) = self.editors.get_mut(&id) {
                doc.diags = diags.into_iter().map(|d| (d.line as usize, d.severity, d.message)).collect();
            }
        }
        // 정의 이동 응답 폴링 — 도착하면 해당 파일을 열어 그 줄로 점프.
        if let (Some(reqid), Some(c)) = (self.lsp.pending_def, &self.lsp.client) {
            if let Some(reply) = c.take_definition(reqid) {
                self.lsp.pending_def = None;
                match reply {
                    Some(def) => {
                        self.open_editor_local(def.path.clone());
                        if let Some(d) = self.editors.values_mut().find(|d| d.path == def.path) {
                            d.jump_to_line(def.line as usize);
                        }
                    }
                    None => self.notify = Some((nabi_i18n::tr(self.lang, "lsp.nodef").to_string(), Instant::now())),
                }
            }
        }
    }

    /// 팔레트 "정의로 이동": 포커스된 rs 문서의 커서 위치로 definition 요청.
    pub(crate) fn lsp_goto_definition(&mut self) {
        let Some(p) = self.focused_pane() else { return };
        let Some(doc) = self.editors.get(&p) else { return };
        if !lsp_doc(doc) {
            return;
        }
        let Some(c) = &self.lsp.client else {
            self.notify = Some((nabi_i18n::tr(self.lang, "lsp.off").to_string(), Instant::now()));
            return;
        };
        let (line, col) = lsp_pos(&doc.text, doc.cur_off);
        self.lsp.pending_def = c.request_definition(&doc.path, line, col);
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
