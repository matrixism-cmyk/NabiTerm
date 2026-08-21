//! 편집 문서 저장 — 저장 바이트 생성·다른 이름으로 저장·인코딩/EOL 변환·저장 시 정리.
//! 문서 모델과 열기는 editor.rs / editoropen.rs.

use crate::app::NabiApp;
use crate::editor::EditorDoc;
use nabi_proto::Command;
use std::time::Instant;
/// 저장 바이트: HEX 버퍼 > rope(원본 EOL 복원) > 텍스트 버퍼 순.
fn doc_bytes(doc: &EditorDoc) -> Vec<u8> {
    if let Some(h) = &doc.hex {
        h.bytes.clone()
    } else if let Some(eb) = &doc.edit {
        eb.to_bytes()
    } else {
        doc.text.clone().into_bytes()
    }
}

/// 저장 시 포맷(순수): trim=각 줄 후행 공백·탭 제거(\r\n 보존), final_nl=파일 끝 개행 보장.
/// VS Code files.trimTrailingWhitespace / insertFinalNewline 대응.
pub(crate) fn format_on_save(text: &str, trim: bool, final_nl: bool, eol: &str) -> String {
    let mut out = if trim {
        text.split('\n').map(|l| {
            let cr = l.ends_with('\r');
            let b = l.trim_end_matches([' ', '\t', '\r']);
            if cr { format!("{b}\r") } else { b.to_string() }
        }).collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    };
    if final_nl && !out.is_empty() && !out.ends_with('\n') {
        out.push_str(if eol == "CRLF" { "\r\n" } else if eol == "CR" { "\r" } else { "\n" });
    }
    out
}

impl NabiApp {
    /// 로컬 파일을 설정에 따라 내장 탭/외부 편집기로 연다(브라우저 "편집").
    pub(crate) fn edit_local_dispatch(&mut self, name: String) {
        let path = self.browser.path.join(&name);
        if self.config.terminal.editor_builtin {
            self.open_editor_local(path);
        } else {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", &path.to_string_lossy()])
                .spawn();
        }
    }

    /// 원격 파일을 설정에 따라 내장 탭/외부 편집기로 연다(SFTP "편집").
    pub(crate) fn edit_remote_dispatch(&mut self, name: String) {
        if self.config.terminal.editor_builtin {
            self.open_editor_remote(name);
        } else {
            self.edit_remote(name);
        }
    }

    /// 다른 이름으로 저장: 네이티브 대화상자로 경로를 받아 버퍼를 쓰고, 그 파일로 전환한다.
    pub(crate) fn save_editor_as(&mut self, pane: nabi_types::PaneId) {
        let cur = match self.editors.get(&pane) {
            Some(d) => d.path.clone(),
            None => return,
        };
        let mut dlg = rfd::FileDialog::new();
        if let Some(dir) = cur.parent() {
            dlg = dlg.set_directory(dir);
        }
        if let Some(name) = cur.file_name().and_then(|n| n.to_str()) {
            dlg = dlg.set_file_name(name);
        }
        let Some(path) = dlg.save_file() else { return };
        self.apply_save_format(pane); // VS Code식 저장 시 정리.
        let Some(doc) = self.editors.get_mut(&pane) else { return };
        let data = doc_bytes(doc);
        let msg = match std::fs::write(&path, &data) {
            Ok(()) => {
                doc.title = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                doc.path = path;
                doc.remote = None; // 로컬 파일로 전환.
                doc.dirty = false;
                if let Some(eb) = doc.edit.as_mut() { eb.mark_saved(); }
                if let Some(h) = doc.hex.as_mut() { h.dirty = false; }
                format!("\u{2713} {}", doc.title)
            }
            Err(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, Instant::now()));
        if let Some(p) = self.editors.get(&pane).map(|d| d.path.clone()) { self.record_editor_mtime(pane, &p); } // 외부 변경 기준.
    }

    /// 파일을 지정 인코딩으로 다시 읽어 버퍼를 교체한다(상태바 인코딩 드롭다운).
    pub(crate) fn reload_editor_encoding(&mut self, pane: nabi_types::PaneId, label: String) {
        let path = match self.editors.get(&pane) {
            Some(d) => d.path.clone(),
            None => return,
        };
        if let Ok(bytes) = std::fs::read(&path) {
            let (text, encoding, eol) = crate::editload::decode_with(&bytes, &label);
            if let Some(d) = self.editors.get_mut(&pane) {
                d.text = text;
                d.encoding = encoding;
                d.eol = eol;
                d.dirty = false;
            }
        }
    }

    /// 버퍼의 줄 끝을 지정 형식으로 변환한다(상태바 EOL 드롭다운). 먼저 LF로 정규화 후 적용.
    pub(crate) fn convert_editor_eol(&mut self, pane: nabi_types::PaneId, eol: &'static str) {
        let Some(d) = self.editors.get_mut(&pane) else { return };
        let lf = d.text.replace("\r\n", "\n").replace('\r', "\n");
        d.text = match eol {
            "CRLF" => lf.replace('\n', "\r\n"),
            "CR" => lf.replace('\n', "\r"),
            _ => lf,
        };
        d.eol = eol;
        d.dirty = true;
    }

    /// 저장 직전 포맷(후행공백 제거·최종 개행) — 설정 시 plain 텍스트 문서에만 적용(HEX/대용량 제외).
    fn apply_save_format(&mut self, pane: nabi_types::PaneId) {
        let (trim, fnl) = (self.editor_config.trim_on_save, self.editor_config.final_newline);
        if !trim && !fnl {
            return;
        }
        if let Some(d) = self.editors.get_mut(&pane) {
            if d.hex.is_none() && d.edit.is_none() && d.big.is_none() {
                d.text = format_on_save(&d.text, trim, fnl, d.eol);
            }
        }
    }

    /// 한 에디터 탭의 버퍼를 저장한다(로컬=파일, 원격=임시 쓰기 후 업로드).
    pub(crate) fn save_editor_doc(&mut self, pane: nabi_types::PaneId) {
        let Some(doc) = self.editors.get(&pane) else { return };
        // 찾기 필터가 켜져 있으면 doc.text는 "일치하는 줄만" 남은 부분집합이다.
        // 그대로 저장하면 나머지 줄이 사라지므로(자동저장 포함) 막고 필터 해제를 안내한다.
        if doc.find.filter_backup.is_some() {
            self.notify = Some((
                format!("\u{26a0} {}", nabi_i18n::tr(self.lang, "find.filtersave")),
                Instant::now(),
            ));
            return;
        }
        if doc.path.as_os_str().is_empty() {
            self.save_editor_as(pane); // untitled(메모리 문서) → 다른 이름으로 저장 대화상자.
            return;
        }
        self.apply_save_format(pane); // VS Code식 저장 시 정리.
        let Some(doc) = self.editors.get(&pane) else { return };
        let data = doc_bytes(doc);
        let (path, remote, title) = (doc.path.clone(), doc.remote.clone(), doc.title.clone());
        let msg = match std::fs::write(&path, &data) {
            Ok(()) => {
                if let Some((id, rp)) = remote {
                    self.orch.send(Command::SftpUpload {
                        id,
                        xfer: crate::sftpxfer::XFER_NONE,
                        local: path.to_string_lossy().into_owned(),
                        remote: rp.clone(),
                    });
                    format!("\u{2191} {rp}")
                } else {
                    format!("\u{2713} {title}")
                }
            }
            Err(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, Instant::now()));
        self.record_editor_mtime(pane, &path); // 자기 저장을 외부 변경으로 오인하지 않도록 갱신.
        if let Some(d) = self.editors.get_mut(&pane) {
            d.dirty = false;
            if let Some(eb) = d.edit.as_mut() { eb.mark_saved(); }
            if let Some(h) = d.hex.as_mut() { h.dirty = false; }
        }
    }
}

#[cfg(test)]
mod save_fmt_tests {
    use super::format_on_save;

    #[test]
    fn trims_and_final_newline() {
        // 후행 공백·탭 제거(\r\n 보존), 끝 개행 보장.
        assert_eq!(format_on_save("a  \nb\t\n", true, false, "LF"), "a\nb\n");
        assert_eq!(format_on_save("x  \r\ny \r\n", true, false, "CRLF"), "x\r\ny\r\n");
        assert_eq!(format_on_save("nofinal", false, true, "LF"), "nofinal\n");
        assert_eq!(format_on_save("crlf", false, true, "CRLF"), "crlf\r\n");
        assert_eq!(format_on_save("keep", false, false, "LF"), "keep"); // 둘 다 off=무변화.
        assert_eq!(format_on_save("", true, true, "LF"), ""); // 빈 파일은 개행 추가 안 함.
    }
}
