//! 에디터 하단 상태바 — Ln/Col · 선택 길이 · 글자/줄/바이트 · 인코딩 · EOL · 줌.
//! 인코딩/EOL은 클릭 드롭다운으로 변환(선택값을 EditorAct로 돌려준다). editortab.rs에서 분리.

use crate::editor::EditorDoc;
/// 경고 칩 색(앱 theme_ui::BROADCAST과 동일 앰버 — 크레이트 분리로 로컬 상수).
const AMBER: egui::Color32 = egui::Color32::from_rgb(255, 176, 32);
use nabi_i18n::{tr, Lang};

/// 줄 끝 변환 선택지.
const EOLS: [&str; 3] = ["LF", "CRLF", "CR"];

/// 구문 강조 언어 선택 항목(자동 + 흔한 언어). doc.syntax_ext를 직접 설정. 보기 메뉴·상태바 공용(DRY).
pub fn syntax_lang_picker(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) {
    if ui.selectable_label(doc.syntax_ext.is_none(), tr(lang, "nabipad.syntaxauto")).clicked() { doc.syntax_ext = None; ui.close(); }
    for (name, ext) in [("Rust", "rs"), ("Python", "py"), ("JavaScript", "js"), ("TypeScript", "ts"), ("JSON", "json"), ("TOML", "toml"), ("YAML", "yaml"), ("Markdown", "md"), ("HTML", "html"), ("CSS", "css"), ("Shell", "sh"), ("C", "c"), ("C++", "cpp"), ("Go", "go"), ("SQL", "sql"), ("Ruby", "rb"), ("PHP", "php"), ("Java", "java"), ("Lua", "lua")] {
        if ui.selectable_label(doc.syntax_ext.as_deref() == Some(ext), name).clicked() { doc.syntax_ext = Some(ext.to_string()); ui.close(); }
    }
}

/// 하단 상태바를 그린다. `cur`=(Ln, Col, 선택 글자수). 반환: (재디코드 인코딩, 변환 EOL).
pub fn editor_status(
    ui: &mut egui::Ui,
    doc: &mut EditorDoc,
    cur: (usize, usize, usize),
    lang: Lang,
) -> (Option<String>, Option<&'static str>) {
    let (chars, lines) = doc.text_stats(); // 길이 변화 시에만 재스캔(D 성능).
    let bytes = crate::humanfmt::human(doc.text.len() as u64);
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
        if let Some(alt) = crate::encdetect::suggest_alt(&doc.encoding, crate::encdetect::replacement_ratio(&sample)) {
            let chip = egui::RichText::new(format!("\u{26a0} {alt}?")).color(AMBER);
            if ui.selectable_label(false, chip).on_hover_text(tr(lang, "status.encsuggest")).clicked() {
                set_enc = Some(alt.to_string());
            }
        }
        ui.separator();
        // 섞인 파일은 종류 하나만 보이면 안 된다 — 저장할 때 온 파일이 그 하나로
        // 바뀌는데, 그 사실이 화면 어디에도 없으면 사용자는 모르고 지나간다.
        let eol_label = match doc.eols.mixed() {
            true => format!("\u{26a0} {}", doc.eols.label()),
            false => doc.eol.to_string(),
        };
        ui.menu_button(eol_label, |ui| {
            for e in EOLS {
                if ui.selectable_label(doc.eol == e, e).clicked() {
                    set_eol = Some(e);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(match doc.eols.mixed() {
            true => tr(lang, "editor.eol.mixed").to_string(),
            false => tr(lang, "editor.eol").to_string(),
        });
        ui.separator();
        // 현재 구문 강조 언어 — 클릭하면 언어 선택(VS Code 언어 모드). syntax_ext 우선(lang_ext).
        let synlang = doc.lang_ext();
        ui.menu_button(synlang, |ui| syntax_lang_picker(ui, doc, lang)).response.on_hover_text(tr(lang, "nabipad.syntaxlang"));
        ui.separator();
        ui.label(format!("{}px", doc.font_size as i32));
        // LSP 서버 상태(rs 문서): 시작 중=모래시계, 준비=번개(호버 설명).
        match doc.lsp_state {
            1 => { ui.separator(); ui.weak("\u{23f3} RA").on_hover_text(tr(lang, "lsp.starting")); }
            2 => { ui.separator(); ui.colored_label(egui::Color32::from_rgb(120, 200, 140), "\u{26a1} RA").on_hover_text(tr(lang, "lsp.ready")); }
            _ => {}
        }
        // LSP 진단 요약(T6-4): 오류/경고 수 + 커서 줄의 첫 진단 메시지.
        if !doc.diags.is_empty() {
            let errs = doc.diags.iter().filter(|(_, s, _)| *s == 1).count();
            let warns = doc.diags.len() - errs;
            ui.separator();
            let red = egui::Color32::from_rgb(235, 80, 80);
            let chip = |ui: &mut egui::Ui, c, s: String| ui.selectable_label(false, egui::RichText::new(s).color(c)).on_hover_text(tr(lang, "lsp.diags")).clicked();
            let mut clicked = false;
            if errs > 0 { clicked |= chip(ui, red, format!("\u{2717} {errs}")); }
            if warns > 0 { clicked |= chip(ui, AMBER, format!("\u{26a0} {warns}")); }
            if clicked { doc.diag_popup = true; } // 클릭 → 진단 목록 팝업.
            if let Some((_, _, msg)) = doc.diags.iter().find(|(l, _, _)| *l == doc.cur_line) {
                let short: String = msg.chars().take(70).collect();
                ui.weak(short).on_hover_text(msg);
            }
        }
    });
    (set_enc, set_eol)
}
