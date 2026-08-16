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

    /// nabiPad 자체 설정 창을 주어진 ctx에 그린다(분리 창=vctx·도크=메인). 닫으면 nabipad.toml 저장·적용.
    pub(crate) fn render_editor_settings(&mut self, ctx: &egui::Context) {
        if self.editor_settings_for.is_none() {
            return;
        }
        let (mut open, lang) = (true, self.lang);
        egui::Window::new(format!("nabiPad \u{2014} {}", tr(lang, "menu.settings")))
            .open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| crate::settingsui::editor_settings_body(ui, &mut self.editor_config, lang));
        if !open {
            self.editor_settings_for = None;
            let _ = nabi_config::save(&self.editor_config_path, &self.editor_config);
            crate::editorsyntax::set_theme(self.editor_config.theme.clone());
            crate::editorsyntax::set_ext_map(self.editor_config.ext_map.clone());
        }
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
        let _ = nabi_config::save(&self.editor_config_path, &self.editor_config);
    }
}

impl crate::app::NabiApp {
    /// 텍스트↔HEX 편집 모드를 전환한다. HEX→텍스트는 현재 바이트를 인코딩으로 디코드,
    /// 텍스트→HEX는 현재 텍스트를 바이트로 적재한다(미저장 변경 유지).
    pub(crate) fn toggle_editor_hex(&mut self, pane: nabi_types::PaneId) {
        let Some(d) = self.editors.get_mut(&pane) else { return };
        if let Some(h) = d.hex.take() {
            let (text, encoding, eol) = nabi_editor::editload::decode(&h.bytes);
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
