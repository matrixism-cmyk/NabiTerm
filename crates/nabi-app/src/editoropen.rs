//! nabiPad 파일 열기 라우팅 — 이진=HEX, 초대용량=뷰어, 대용량=rope, 그 외=텍스트.
//! 패널 추가(도크 탭/분리 창)와 원격 다운로드 적재도 여기서 처리한다. editor.rs에서 분리.

use crate::app::NabiApp;
use crate::editor::{file_name, EditorDoc};
use nabi_i18n::tr;
use nabi_proto::Command;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 이 크기를 넘으면 rope 대신 **흘려 읽기 편집기**로 연다.
///
/// rope는 문서를 통째로 RAM에 올린다. 64MB 로그를 열면 64MB를 먹고 여는 데도 몇 초가 걸린다.
/// 흘려 읽기 편집기는 즉시 열리고 메모리가 편집 횟수에 비례한다. 대신 접기·미니맵·구문
/// 강조가 없는데, 이 크기에서는 어차피 구문 강조가 꺼지고(`editorhl::MAX_SYNTAX_BYTES`=2MB)
/// 나머지도 느리다. 그래서 이 선 위에서는 흘려 읽기 쪽이 모든 면에서 낫다.
const HUGE_THRESHOLD: u64 = 64_000_000;

impl NabiApp {
    /// 포커스 pane을 새 nabiPad 문서로 연다 — 선택 영역이 있으면 그 부분만, 없으면 전체 스크롤백.
    pub(crate) fn edit_scrollback_in_pad(&mut self) {
        let Some(p) = self.focused_pane() else { return };
        let text = self.selection_text().filter(|t| !t.is_empty())
            .or_else(|| self.orch.panes.read().ok().and_then(|m| m.get(&p).cloned()).and_then(|v| v.model.lock().ok().map(|md| md.dump_text(1_000_000))));
        let Some(text) = text else { return };
        let mut doc = EditorDoc::make(tr(self.lang, "cmd.scrollbackdoc").to_string(), PathBuf::new(), None, text, true, self.font_size, "UTF-8".into(), "LF");
        doc.dirty = true; self.add_editor_tab(doc); // 메모리 문서 — 저장 시 다른 이름으로.
    }

    /// 파일 빠른 미리보기(E9) — 앞 64KB만 디코드해 최대 200줄을 팝업에 띄운다(열지 않고 확인).
    pub(crate) fn open_file_preview(&mut self, path: PathBuf) {
        let title = file_name(&path);
        let body = std::fs::read(&path).ok().map(|b| {
            let cap = b.len().min(64 * 1024);
            crate::editload::decode(&b[..cap]).0
        }).unwrap_or_default();
        let text: String = body.lines().take(200).collect::<Vec<_>>().join("\n");
        self.file_preview = Some((title, text));
    }

    /// 미리보기 팝업 렌더(스크롤·모노스페이스). 닫으면 상태 해제.
    pub(crate) fn file_preview_modal(&mut self, ctx: &egui::Context) {
        let Some((title, text)) = self.file_preview.clone() else { return };
        let mut open = true;
        egui::Window::new(format!("\u{1f441} {title}"))
            .open(&mut open).resizable(true).default_size([640.0, 480.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&text).monospace()).wrap_mode(egui::TextWrapMode::Extend));
                });
            });
        if !open {
            self.file_preview = None;
        }
    }

    /// 에디터 닫기 요청(파일 메뉴/X). 변경사항 있으면 저장 확인 모달, 아니면 즉시 닫기(#4).
    /// 같은 경로가 이미 열려 있으면 그 탭으로 포커스(true면 새로 열 필요 없음).
    /// 분리 창으로 떠 있는 경우엔 중복 생성만 막는다(OS 창 포커스는 강제하지 않음).
    fn focus_existing_editor(&mut self, path: &Path) -> bool {
        let found = self.editors.iter().find(|(_, d)| d.path == path).map(|(p, _)| *p);
        if let Some(p) = found {
            if let Some(loc) = self.dock.find_tab(&p) {
                let _ = self.dock.set_active_tab(loc);
            }
            return true;
        }
        false
    }

    /// 문서를 새 에디터 패널로 추가한다. nabiPad 설정 `open_in_window`면 분리 창으로,
    /// 아니면 도크 탭으로 연다(browser_tabs와 동일 방식).
    pub(crate) fn add_editor_tab(&mut self, mut doc: EditorDoc) {
        let p = nabi_types::next_pane_id();
        // nabiPad 설정(nabipad.toml)을 새 문서에 적용: 메뉴바·강조·줄바꿈·공백 표시.
        let ec = &self.editor_config;
        doc.show_menu = ec.show_menu_bar; doc.highlight = ec.syntax_highlight;
        doc.wrap = ec.word_wrap; doc.show_ws = ec.show_whitespace;
        self.editors.insert(p, doc);
        // 외부 변경 감시 기준 시각은 일반 로컬 텍스트 문서에만 기록한다.
        let plain = self.editors.get(&p).filter(|d| {
            d.hex.is_none() && d.edit.is_none() && d.big.is_none() && d.remote.is_none()
        });
        if let Some(path) = plain.map(|d| d.path.clone()).filter(|p| !p.as_os_str().is_empty()) {
            self.record_editor_mtime(p, &path);
        }
        if self.editor_config.open_in_window { self.floating.push(p); } else { self.add_pane(p); } // 분리창/도크탭.
    }

    /// 초대용량(EDIT_CAP 초과) → memmap 읽기 전용 뷰어 탭(E4).
    fn open_big_viewer(&mut self, path: PathBuf) {
        if let Some(big) = crate::editbig::BigFile::open(&path) {
            let (title, encoding) = (file_name(&path), big.encoding().to_string());
            self.add_editor_tab(EditorDoc {
                title, path, remote: None, text: String::new(), dirty: false, loaded: true,
                font_size: self.font_size, encoding, eol: "-", highlight: false,
                wrap: false, show_ws: false, readonly: true, big: Some(big), edit: None, huge: None,
                find: Default::default(), show_menu: false, hex: None, stats_cache: (usize::MAX, 0, 0), minimap: false, outline: false, show_lineno: true, bookmarks: Vec::new(), cur_line: 0, syntax_ext: None,
                diags: Vec::new(), cur_off: 0, lsp_info: None, lsp_refs: None, diag_popup: false, rename_open: false,
                lsp_comp: None, comp_anchor: 0, cursor_px: (0.0, 0.0), lsp_state: 0,
            });
        } else {
            self.notify = Some((tr(self.lang, "editor.toobig").to_string(), Instant::now()));
        }
    }

    /// 대용량(2MB~EDIT_CAP) → rope 가상화 편집기 탭(E6).
    ///
    /// rope는 문서를 통째로 RAM에 올리므로 그 위(512MB 초과)는 [`open_huge_editor`]가 맡는다.
    fn open_rope_editor(&mut self, path: PathBuf) {
        match crate::editbuf::EditBuf::open(&path) {
            Some(mut eb) => {
                eb.tab = self.editor_config.tab_size.max(1); // 표시·탭 스톱 기준(설정).
                eb.spaces = self.editor_config.indent_spaces;
                let (title, encoding, eol) = (file_name(&path), eb.enc.clone(), eb.eol);
                let mut doc = EditorDoc::make(title, path, None, String::new(), true, self.font_size, encoding, eol);
                doc.edit = Some(eb);
                self.add_editor_tab(doc);
            }
            None => self.open_huge_editor(path),
        }
    }

    /// 초대용량 → **용량 제한 없는 편집기**(N5). 파일을 메모리에 올리지 않는다.
    ///
    /// 예전에는 여기서 읽기 전용 뷰어로 떨어뜨렸다("대용량이면 편집을 눌러도 뷰어로 열린다" —
    /// 사용자 지적). 이제는 편집이 된다. 그래도 열 수 없는 경우(CR 전용 줄바꿈 등)에만
    /// 뷰어로 물러난다.
    fn open_huge_editor(&mut self, path: PathBuf) {
        let Ok(data) = nabi_editor::textdata::TextData::open(&path) else {
            return self.open_big_viewer(path);
        };
        let (title, encoding, eol) = (file_name(&path), data.encoding().to_string(), data.eol);
        // 어떤 편집기가 골라졌는지 로그에 남긴다 — 사후 진단과 e2e 확인에 쓴다.
        tracing::info!(target: "editor", file = %path.display(), bytes = data.total(), lines = data.lines(), "huge-editor");
        let mut doc = EditorDoc::make(title, path, None, String::new(), true, self.font_size, encoding, eol);
        doc.highlight = false; // 이 편집기에는 구문 강조가 없다(문서 전체를 훑어야 한다).
        doc.huge = Some(nabi_editor::textbuf::TextBuf::new(data));
        self.add_editor_tab(doc);
    }

    /// 로컬 파일을 강제로 HEX 편집기로 연다(텍스트 파일도 바이트로 — 브라우저 'HEX로 열기').
    pub(crate) fn open_local_as_hex(&mut self, path: PathBuf) {
        if !self.focus_existing_editor(&path) {
            self.open_hex(path);
        }
    }

    /// 이진 파일을 HEX 편집기 탭으로 연다(N3). 상한 초과면 읽기 전용 뷰어로 폴백.
    fn open_hex(&mut self, path: PathBuf) {
        match crate::edithex::HexBuf::open(&path) {
            Some(hb) => {
                let mut d = EditorDoc::make(file_name(&path), path, None, String::new(), true, self.font_size, "binary".into(), "-");
                d.hex = Some(hb);
                self.add_editor_tab(d);
            }
            None => self.open_big_viewer(path), // HEX 상한 초과 → 읽기 전용 가상화 뷰어.
        }
    }

    /// 최근 연 파일 목록(nabiPad)에 경로를 올린다(중복 제거·최신 우선·최대 12개, 저장).
    fn push_recent_file(&mut self, path: &Path) {
        let s = path.to_string_lossy().into_owned();
        let r = &mut self.editor_config.recent_files;
        r.retain(|x| x != &s);
        r.insert(0, s);
        r.truncate(12);
        let _ = nabi_config::save(&self.editor_config_path, &self.editor_config);
    }

    /// 로컬 파일을 내장 에디터 탭으로 연다. 이진=HEX, 대용량=rope 편집기(512MB 초과만 읽기 전용). (링크 메뉴에서도 호출.)
    pub(crate) fn open_editor_local(&mut self, path: PathBuf) {
        self.push_recent_file(&path); // 최근 파일 기록(A5).
        if self.focus_existing_editor(&path) {
            return;
        }
        // 이진 파일은 크기와 무관하게 HEX 편집기로(앞부분만 보고 판정 — 대용량도 빠름).
        if crate::edithex::peek_is_binary(&path) {
            self.open_hex(path);
            return;
        }
        let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if len > HUGE_THRESHOLD {
            self.open_huge_editor(path); // 흘려 읽기 편집기 — 파일을 메모리에 올리지 않는다.
            return;
        }
        if len > crate::editbig::BIG_THRESHOLD {
            self.open_rope_editor(path); // 2MB~64MB는 rope(접기·미니맵을 쓸 수 있다).
            return;
        }
        match std::fs::read(&path) {
            Ok(bytes) => {
                let (text, encoding, eol) = crate::editload::decode(&bytes);
                let (title, fs) = (file_name(&path), self.font_size);
                self.add_editor_tab(EditorDoc::make(title, path, None, text, true, fs, encoding, eol));
            }
            Err(e) => self.notify = Some((format!("\u{2715} {e}"), Instant::now())),
        }
    }

    /// 원격 파일을 임시로 내려받아 내장 에디터 탭으로 연다(다운로드 후 on_edit_download가 적재).
    pub(crate) fn open_editor_remote(&mut self, name: String) {
        let Some(id) = self.sftp.id else { return };
        let remote = crate::sftppath::join_path(&self.sftp.path, &name);
        let temp = std::env::temp_dir().join(format!("nabi-editor-{name}"));
        if self.focus_existing_editor(&temp) {
            return;
        }
        self.orch.send(Command::SftpDownload {
            id,
            xfer: crate::sftpxfer::XFER_NONE,
            remote: remote.clone(),
            local: temp.to_string_lossy().into_owned(),
            resume: 0,
        });
        self.sftp.status = tr(self.lang, "sftp.downloading").to_string();
        let fs = self.font_size;
        self.add_editor_tab(EditorDoc::make(
            name, temp, Some((id, remote)), String::new(), false, fs, "UTF-8".into(), "LF",
        ));
    }

    /// 현재 텍스트를 지정 인코딩으로 인코딩해 저장한다(일반 텍스트 문서 전용, 원격이면 업로드).
    pub(crate) fn save_with_encoding(&mut self, pane: nabi_types::PaneId, label: String) {
        let (data, path, remote) = match self.editors.get(&pane) {
            Some(d) if d.hex.is_none() && d.edit.is_none() && d.big.is_none() => {
                (crate::editload::encode(&d.text, &label), d.path.clone(), d.remote.clone())
            }
            _ => return,
        };
        let msg = match std::fs::write(&path, &data) {
            Ok(()) => {
                if let Some((id, rp)) = &remote {
                    self.orch.send(Command::SftpUpload { id: *id, xfer: crate::sftpxfer::XFER_NONE, local: path.to_string_lossy().into_owned(), remote: rp.clone() });
                }
                format!("\u{2713} {label}")
            }
            Err(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, Instant::now()));
        if let Some(d) = self.editors.get_mut(&pane) {
            d.dirty = false;
            d.encoding = label;
        }
    }

    /// 일반 텍스트 문서를 디스크에서 다시 읽어 버퍼를 교체한다(되돌리기 — 미저장 변경 폐기).
    pub(crate) fn reload_editor_doc(&mut self, pane: nabi_types::PaneId) {
        let path = match self.editors.get(&pane) {
            Some(d) if d.hex.is_none() && d.edit.is_none() && d.big.is_none() => d.path.clone(),
            _ => return, // HEX/대용량은 대상 아님.
        };
        if let Ok(bytes) = std::fs::read(&path) {
            let (text, encoding, eol) = crate::editload::decode(&bytes);
            if let Some(d) = self.editors.get_mut(&pane) {
                d.text = text;
                d.encoding = encoding;
                d.eol = eol;
                d.dirty = false;
                d.stats_cache = (usize::MAX, 0, 0); // 통계 캐시 무효화.
            }
            self.record_editor_mtime(pane, &path); // 리로드 후 외부 변경 기준 갱신(재감지 방지).
        }
    }

    /// SFTP 다운로드 완료 공통 처리: 외부 편집기 오픈 또는 내장 에디터 탭 적재.
    pub(crate) fn on_edit_download(&mut self, local: &str) {
        self.open_edit(local); // 외부 편집기 감시 항목.
        // 내장 에디터: 적재 대기 중인 탭이 이 경로면 내용 채움.
        if let Some(doc) = self.editors.values_mut().find(|d| !d.loaded && d.path.to_string_lossy() == local) {
            if let Ok(b) = std::fs::read(&doc.path) {
                let (text, encoding, eol) = crate::editload::decode(&b);
                doc.text = text;
                doc.encoding = encoding;
                doc.eol = eol;
            }
            doc.loaded = true;
        }
    }
}
