//! 에디터 하단 상태바 — Ln/Col · 선택 길이 · 글자/줄/바이트 · 인코딩 · EOL · 줌.
//! 인코딩/EOL은 클릭 드롭다운으로 변환(선택값을 EditorAct로 돌려준다). editortab.rs에서 분리.

use crate::editor::EditorDoc;
use nabi_i18n::{tr, Lang};

/// 줄 끝 변환 선택지.
const EOLS: [&str; 3] = ["LF", "CRLF", "CR"];

/// 구문 강조 언어 선택 항목(자동 + 흔한 언어). doc.syntax_ext를 직접 설정. 보기 메뉴·상태바 공용(DRY).
pub(crate) fn syntax_lang_picker(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) {
    if ui.selectable_label(doc.syntax_ext.is_none(), tr(lang, "nabipad.syntaxauto")).clicked() { doc.syntax_ext = None; ui.close_menu(); }
    for (name, ext) in [("Rust", "rs"), ("Python", "py"), ("JavaScript", "js"), ("TypeScript", "ts"), ("JSON", "json"), ("TOML", "toml"), ("YAML", "yaml"), ("Markdown", "md"), ("HTML", "html"), ("CSS", "css"), ("Shell", "sh"), ("C", "c"), ("C++", "cpp"), ("Go", "go"), ("SQL", "sql"), ("Ruby", "rb"), ("PHP", "php"), ("Java", "java"), ("Lua", "lua")] {
        if ui.selectable_label(doc.syntax_ext.as_deref() == Some(ext), name).clicked() { doc.syntax_ext = Some(ext.to_string()); ui.close_menu(); }
    }
}

/// 하단 상태바를 그린다. `cur`=(Ln, Col, 선택 글자수). 반환: (재디코드 인코딩, 변환 EOL).
pub(crate) fn editor_status(
    ui: &mut egui::Ui,
    doc: &mut EditorDoc,
    cur: (usize, usize, usize),
    lang: Lang,
) -> (Option<String>, Option<&'static str>) {
    let (chars, lines) = doc.text_stats(); // 길이 변화 시에만 재스캔(D 성능).
    let bytes = crate::browserfs::human(doc.text.len() as u64);
    let (mut set_enc, mut set_eol) = (None, None);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Ln {}, Col {}", cur.0, cur.1));
        if cur.2 > 0 {
            ui.separator();
            ui.label(format!("Sel {}", cur.2));
        }
        ui.separator();
        ui.label(format!("{chars} chars \u{00b7} {lines} lines \u{00b7} {bytes}"));
        ui.separator();
        ui.menu_button(&doc.encoding, |ui| {
            if let Some(e) = crate::encodings::encoding_menu(ui, &doc.encoding) {
                set_enc = Some(e);
            }
        });
        // P11: 열린 파일이 깨져 보이면(U+FFFD 비율↑) 대안 인코딩으로 다시 열기 제안(P4 SSOT 재사용).
        let sample: String = doc.text.chars().take(4000).collect();
        if let Some(alt) = crate::encsuggest::suggest_alt(&doc.encoding, crate::encsuggest::replacement_ratio(&sample)) {
            let chip = egui::RichText::new(format!("\u{26a0} {alt}?")).color(crate::theme_ui::BROADCAST);
            if ui.selectable_label(false, chip).on_hover_text(tr(lang, "status.encsuggest")).clicked() {
                set_enc = Some(alt.to_string());
            }
        }
        ui.separator();
        ui.menu_button(doc.eol, |ui| {
            for e in EOLS {
                if ui.selectable_label(doc.eol == e, e).clicked() {
                    set_eol = Some(e);
                    ui.close_menu();
                }
            }
        })
        .response
        .on_hover_text(tr(lang, "editor.eol"));
        ui.separator();
        // 현재 구문 강조 언어 — 클릭하면 언어 선택(VS Code 언어 모드). syntax_ext 우선(lang_ext).
        let synlang = doc.lang_ext();
        ui.menu_button(synlang, |ui| syntax_lang_picker(ui, doc, lang)).response.on_hover_text(tr(lang, "nabipad.syntaxlang"));
        ui.separator();
        ui.label(format!("{}px", doc.font_size as i32));
    });
    (set_enc, set_eol)
}
