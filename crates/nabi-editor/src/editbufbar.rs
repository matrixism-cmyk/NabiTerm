//! 대용량 rope 편집기 상단 도구막대 + 하단 상태바 — editbufview에서 분리(라인 한도).

use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const WARN: egui::Color32 = egui::Color32::from_rgb(240, 180, 60);

/// 상단 도구막대: 메뉴 토글·저장·undo/redo·강조·읽기전용·수정 표시.
pub(crate) fn toolbar(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang, act: &mut EditorAct) {
    ui.horizontal(|ui| {
        if !doc.show_menu && ui.button("\u{2630}").on_hover_text(tr(lang, "nabipad.menu.show")).clicked() {
            act.toggle_menu_bar = true;
        }
        if ui.button(format!("\u{1f4be} {}", tr(lang, "editor.save"))).clicked() {
            act.save = true;
        }
        if ui.button(tr(lang, "editor.saveas")).clicked() {
            act.save_as = true;
        }
        // T4-1 패닉 감사: edit가 없는 프레임이 와도 UI 스레드가 죽지 않게 unwrap 금지.
        if let Some(eb) = doc.edit.as_mut() {
            if ui.button("\u{21b6}").on_hover_text(tr(lang, "editor.undo")).clicked() {
                eb.undo();
            }
            if ui.button("\u{21b7}").on_hover_text(tr(lang, "editor.redo")).clicked() {
                eb.redo();
            }
        }
        ui.toggle_value(&mut doc.highlight, "\u{1f3a8}").on_hover_text(tr(lang, "editor.highlight"));
        ui.toggle_value(&mut doc.readonly, "\u{1f512}").on_hover_text(tr(lang, "editor.readonly"));
        if doc.edit.as_ref().is_some_and(|e| e.dirty) {
            ui.colored_label(WARN, "\u{25cf}");
        }
    });
}

/// 하단 상태바: Ln/Col · 선택수 · 줄/바이트 · 인코딩 · EOL · 줌.
pub(crate) fn eb_status(ui: &mut egui::Ui, doc: &EditorDoc, cur: (usize, usize), sel: usize, lang: Lang) {
    let Some(eb) = doc.edit.as_ref() else { return };
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Ln {}, Col {}", cur.0 + 1, cur.1 + 1));
        if sel > 0 {
            ui.separator();
            ui.label(format!("Sel {sel}"));
        }
        // 커서가 여럿이면 몇 개인지 보여 준다 — 지난 배치에 늘리는 길을 냈는데 개수가
        // 안 보여서, 몇 군데를 고치고 있는지 모른 채 타자를 치게 됐다.
        if eb.sel.len() > 1 {
            ui.separator();
            ui.label(format!("\u{25c9} {}", eb.sel.len())).on_hover_text(tr(lang, "edit.cursors"));
        }
        ui.separator();
        ui.label(format!("{} lines \u{00b7} {}", eb.rope.len_lines(), crate::humanfmt::human(eb.rope.len_bytes() as u64)));
        ui.separator();
        ui.label(&eb.enc);
        ui.separator();
        ui.label(eb.eol);
        ui.separator();
        ui.label(format!("{}px", doc.font_size as i32));
        ui.separator();
        ui.label(tr(lang, "editor.bigedit"));
    });
}

