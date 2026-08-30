//! nabiPad 앱 글루(T5-1) — 에디터 코어(nabi-editor)와 NabiApp을 잇는 impl 모음.
//! 크레이트 분리 시 editorctx/editormenu/edithex에서 추출했다(코어는 앱을 모른다).

use crate::app::NabiApp;
use crate::editor::EditorDoc;
use nabi_editor::edithex::HexBuf;
use nabi_i18n::tr;
use std::path::PathBuf;

impl NabiApp {
    /// nabiPad 빈 새 문서를 연다(메뉴/팔레트에서 바로 실행).
    pub(crate) fn open_empty_pad(&mut self) {
        self.add_editor_tab(EditorDoc::make(tr(self.lang, "nabipad.newdoc").to_string(), PathBuf::new(), None, String::new(), true, self.font_size, "UTF-8".into(), "LF")); // 새 문서 EOL 기본 LF(상태바 표기 일치).
    }

    /// nabiPad 자체 설정 창을 주어진 ctx에 그린다(분리 창=vctx·도크=메인).
    ///
    /// 주 설정 대화상자와 **같은 방식**이다 — 스냅샷을 찍고, 저장·취소 버튼을 주고, 창의
    /// X는 취소로 본다. 같은 제품 안에서 같은 종류의 화면이 다르게 동작하면 그것만으로
    /// 사용자가 헷갈린다(사용자 지적 2026-08-25).
    pub(crate) fn render_editor_settings(&mut self, ctx: &egui::Context) {
        if self.editor_settings_for.is_none() {
            return;
        }
        if self.editor_settings_backup.is_none() {
            self.editor_settings_backup = Some(self.editor_config.clone());
        }
        let (mut open, lang, mut done) = (true, self.lang, None);
        egui::Window::new(format!("nabiPad \u{2014} {}", tr(lang, "menu.settings")))
            .open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                crate::settingsui::editor_settings_body(ui, &mut self.editor_config, lang);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr(lang, "settings.save")).clicked() {
                        done = Some(true);
                    }
                    if ui.button(tr(lang, "qc.cancel")).clicked() {
                        done = Some(false);
                    }
                });
            });
        let Some(save) = done.or(if open { None } else { Some(false) }) else { return };
        let backup = self.editor_settings_backup.take();
        if !save {
            if let Some(b) = backup {
                self.editor_config = b;
            }
        }
        self.editor_settings_for = None;
        self.save_editor_config();
        crate::editorsyntax::set_theme(self.editor_config.theme.clone());
        crate::editorsyntax::set_ext_map(self.editor_config.ext_map.clone());
    }
}

impl NabiApp {
    /// 메뉴바 표시를 토글하고 nabipad.toml에 저장 + 열린 모든 에디터에 즉시 반영.
    pub(crate) fn toggle_editor_menu_bar(&mut self) {
        let v = !self.editor_config.show_menu_bar;
        self.editor_config.show_menu_bar = v;
        for d in self.editors.values_mut() {
            d.show_menu = v;
        }
        self.save_editor_config();
    }
}

impl crate::app::NabiApp {
    /// 텍스트↔HEX 편집 모드를 전환한다. HEX→텍스트는 현재 바이트를 인코딩으로 디코드,
    /// 텍스트→HEX는 현재 텍스트를 바이트로 적재한다(미저장 변경 유지).
    pub(crate) fn toggle_editor_hex(&mut self, pane: nabi_types::PaneId) {
        let Some(d) = self.editors.get_mut(&pane) else { return };
        if let Some(h) = d.hex.take() {
            let (text, encoding, eol) = nabi_editor::editload::decode(&h.bytes());
            d.text = text;
            d.encoding = encoding;
            d.eol = eol;
            d.dirty = h.dirty || d.dirty;
        } else if d.edit.is_none() && d.big.is_none() {
            // 일반 텍스트 버퍼만 HEX로 전환(대용량 rope/뷰어는 제외).
            let mut hb = HexBuf::from_bytes(std::mem::take(&mut d.text).into_bytes());
            hb.dirty = d.dirty;
            d.hex = Some(hb);
            d.encoding = "binary".into();
        }
    }
}
