//! 용량 무제한 편집기의 도구막대 + 상태바 — textview에서 분리(라인 한도).
//!
//! rope 편집기(`editbufbar`)와 짝을 이루되, 이쪽은 **문서 크기를 자랑하지 않는다.** 줄 수와
//! 바이트 수는 인덱스가 이미 알고 있어 공짜지만, 그 밖에 문서 전체를 훑어야 나오는 값
//! (글자 수 따위)은 여기 두지 않는다 — 이 편집기가 존재하는 이유가 그것이다.

use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const WARN: egui::Color32 = egui::Color32::from_rgb(240, 180, 60);
/// 칸 안내 색 — 정보라 눈에 튀지 않게, 그래도 다른 값과 구별되게.
const FIELD: egui::Color32 = egui::Color32::from_rgb(140, 190, 230);

/// 상단 도구막대: 메뉴 토글·저장·되돌리기·읽기전용·수정 표시.
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
        // huge가 없는 프레임이 와도 UI 스레드가 죽지 않게 unwrap 금지(패닉 감사 규칙).
        if let Some(tb) = doc.huge.as_mut() {
            if ui.add_enabled(tb.can_undo(), egui::Button::new("\u{21b6}")).on_hover_text(tr(lang, "editor.undo")).clicked() {
                tb.undo();
            }
            if ui.add_enabled(tb.can_redo(), egui::Button::new("\u{21b7}")).on_hover_text(tr(lang, "editor.redo")).clicked() {
                tb.redo();
            }
        }
        ui.toggle_value(&mut doc.readonly, "\u{1f512}").on_hover_text(tr(lang, "editor.readonly"));
        if doc.huge.as_ref().is_some_and(|t| t.dirty) {
            ui.colored_label(WARN, "\u{25cf}");
        }
    });
}

/// 하단 상태바: Ln/Col · 줄 수 · 크기 · 인코딩 · EOL · 줌 · 이 편집기임을 알리는 표시.
pub(crate) fn status(ui: &mut egui::Ui, doc: &EditorDoc, cur: (usize, usize), sel: bool, lines: usize, lang: Lang) {
    let Some(tb) = doc.huge.as_ref() else { return };
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Ln {}, Col {}", cur.0 + 1, cur.1 + 1));
        // 표처럼 생긴 줄이면 몇 번째 칸인지 함께 보여 준다(배치 AC).
        //
        // **현재 줄과 첫 줄만** 읽는다. 칸 너비를 맞추려면 문서 전체를 봐야 하는데, 이 편집기가
        // 존재하는 이유가 그것을 안 하는 것이다. "몇 번째 칸인가"는 그 두 줄이면 답이 나온다.
        if let Some(h) = csv_hint(tb, cur) {
            ui.separator();
            ui.label(egui::RichText::new(h).color(FIELD))
                .on_hover_text(tr(lang, "editor.csvcol.hint"));
        }
        if sel {
            let (a, b) = tb.selection();
            ui.separator();
            ui.label(format!("Sel {}", crate::humanfmt::human(b - a)));
        }
        ui.separator();
        ui.label(format!("{lines} lines \u{00b7} {}", crate::humanfmt::human(tb.data.total())));
        ui.separator();
        ui.label(tb.data.encoding());
        ui.separator();
        ui.label(tb.data.eol);
        ui.separator();
        ui.label(format!("{}px", doc.font_size as i32));
        ui.separator();
        ui.label(tr(lang, "editor.hugeedit")).on_hover_text(tr(lang, "editor.hugeedit.hint"));
    });
}

/// 현재 줄의 칸 안내(`3/12 user_id`). 표가 아니면 `None`.
///
/// 첫 줄을 머리글로 본다. 커서가 첫 줄에 있으면 머리글을 쓰지 않는다 — 자기 자신을 이름으로
/// 삼으면 "id 칸의 이름은 id"라는 뻔한 말이 나온다.
fn csv_hint(tb: &crate::textbuf::TextBuf, cur: (usize, usize)) -> Option<String> {
    let line = tb.data.line(cur.0);
    let header = (cur.0 > 0 && tb.data.lines() > 1).then(|| tb.data.line(0));
    crate::csvcol::hint(&line, header.as_deref(), cur.1)
}
