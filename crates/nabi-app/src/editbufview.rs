//! 대용량 편집 버퍼(E6)의 가상화 렌더 + 자작 입력 처리. 보이는 줄만 그리고 키/마우스로 편집한다.
//! 등폭 가정으로 열↔x를 환산한다(CJK는 약간 어긋날 수 있음 — v1). 키 처리는 editbufkeys로 분리.

use crate::editbuf::EditBuf;
use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const WARN: egui::Color32 = egui::Color32::from_rgb(240, 180, 60);
const GUTTER: egui::Color32 = egui::Color32::from_rgb(120, 130, 145);
const CARET: egui::Color32 = egui::Color32::from_rgb(220, 225, 245);
// premultiplied는 RGB ≤ alpha라야 정상 반투명 — RGB>alpha면 가산 혼합되어 줄 배경이 순백이 되며 글자를 덮는다.
const CURLINE: egui::Color32 = egui::Color32::from_rgba_premultiplied(22, 22, 22, 22); // 현재 줄 강조(white·a22).

/// 편집 버퍼 도크 탭 본문(도구막대 + 찾기 + 상태바 + 가상화 편집 영역).
pub(crate) fn edit_view(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> EditorAct {
    let mut act = EditorAct::default();
    // nabiPad 메뉴바(표시 설정 시) — 대용량 편집기에서도 동일하게 노출.
    if doc.show_menu {
        crate::editormenu::menu_bar(ui, doc, lang, &mut act, &[]);
    }
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
        let eb = doc.edit.as_mut().unwrap();
        if ui.button("\u{21b6}").on_hover_text(tr(lang, "editor.undo")).clicked() {
            eb.undo();
        }
        if ui.button("\u{21b7}").on_hover_text(tr(lang, "editor.redo")).clicked() {
            eb.redo();
        }
        ui.toggle_value(&mut doc.readonly, "\u{1f512}").on_hover_text(tr(lang, "editor.readonly"));
        if doc.edit.as_ref().unwrap().dirty {
            ui.colored_label(WARN, "\u{25cf}");
        }
    });
    ui.separator();
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::renameui::ctrl_wheel_zoom(ui, over);
    if dz != 0.0 {
        doc.font_size = (doc.font_size + dz).clamp(8.0, 40.0);
    }
    if over && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
        doc.find.open = !doc.find.open;
    }
    if doc.find.open {
        crate::editorfind::find_bar(ui, doc, lang);
        ui.separator();
    }
    let cur = doc.edit.as_ref().unwrap().cursor_line_col();
    let sel = doc.edit.as_ref().unwrap().selected_text().chars().count();
    egui::TopBottomPanel::bottom(ui.id().with("eb_status")).show_inside(ui, |ui| {
        eb_status(ui, doc, cur, sel, lang);
    });
    edit_body(ui, doc);
    act
}

/// 하단 상태바: Ln/Col · 선택수 · 줄/바이트 · 인코딩 · EOL · 줌.
fn eb_status(ui: &mut egui::Ui, doc: &EditorDoc, cur: (usize, usize), sel: usize, lang: Lang) {
    let eb = doc.edit.as_ref().unwrap();
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Ln {}, Col {}", cur.0 + 1, cur.1 + 1));
        if sel > 0 {
            ui.separator();
            ui.label(format!("Sel {sel}"));
        }
        ui.separator();
        ui.label(format!("{} lines \u{00b7} {}", eb.rope.len_lines(), crate::browserfs::human(eb.rope.len_bytes() as u64)));
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

/// 가상화 편집 영역 — 보이는 줄만 그리고, 포커스 시 키/마우스로 편집한다.
fn edit_body(ui: &mut egui::Ui, doc: &mut EditorDoc) {
    let (fsize, readonly) = (doc.font_size, doc.readonly);
    let mono = egui::FontId::monospace(fsize);
    let row_h = ui.fonts(|f| f.row_height(&mono)).max(1.0);
    let char_w = ui.fonts(|f| f.glyph_width(&mono, '0')).max(6.0);
    let scroll_line = doc.find.scroll_to.take();
    let eb = doc.edit.as_mut().unwrap();
    let lc = eb.rope.len_lines();
    let gutter_w = char_w * (lc.to_string().len().max(4) as f32) + 12.0;
    let avail_w = ui.available_width(); // 창을 가득 채우도록 콘텐츠 최소 폭 기준.
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("eb_body");
    if let Some(l) = scroll_line {
        sa = sa.vertical_scroll_offset(l as f32 * row_h);
    }
    sa.show_viewport(ui, |ui, vp| {
        // 보이는 줄 범위 + 그 최장 길이로 콘텐츠 크기 결정(가로 스크롤). 폭은 최소 가용폭만큼 채운다.
        let first = (vp.top() / row_h).floor().max(0.0) as usize;
        let last = ((vp.bottom() / row_h) as usize + 2).min(lc);
        let maxcols = (first..last).map(|i| eb.line_len(i)).max().unwrap_or(0);
        let content_w = (gutter_w + (maxcols as f32 + 2.0) * char_w).max(avail_w);
        ui.set_width(content_w);
        ui.set_height(lc as f32 * row_h);
        let (top, left) = (ui.min_rect().top(), ui.min_rect().left());
        let text_left = left + gutter_w;
        // 포커스/입력. 클릭 영역은 보이는 부분 전체.
        let id = ui.id().with("eb_area");
        let resp = ui.interact(ui.clip_rect(), id, egui::Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() {
            resp.request_focus();
        }
        let focused = resp.has_focus();
        // 마우스 → 커서/선택. 명시적 제스처(클릭/드래그)에만 반응한다 —
        // 단순 누름-유지 프레임에서 선택을 확장하던 버그(클릭 시 한 줄 역상) 수정.
        if let Some(p) = resp.interact_pointer_pos() {
            let pos = hit(eb, p, top, text_left, row_h, char_w);
            let shift = ui.input(|i| i.modifiers.shift);
            if resp.drag_started() {
                if !shift {
                    eb.anchor = Some(pos); // 새 드래그 선택의 시작점.
                }
                eb.cursor = pos;
                eb.undo_open = false;
            } else if resp.dragged() {
                eb.move_to(pos, true); // 드래그 중 선택 확장.
            } else if resp.clicked() {
                if shift {
                    eb.move_to(pos, true); // Shift+클릭 = 현재 위치까지 확장.
                } else {
                    eb.cursor = pos;
                    eb.anchor = Some(pos); // 단순 클릭 = 커서 이동(선택 해제).
                    eb.undo_open = false;
                }
            }
        }
        // 키보드(포커스 + 편집 가능).
        if focused {
            let page = (vp.height() / row_h).max(1.0) as i64;
            crate::editbufkeys::apply_keys(ui, eb, page, readonly);
        }
        // 커서 캐럿 위치(스크롤은 painter 빌림 전에 — &mut ui 필요).
        let (cl, cc) = eb.cursor_line_col();
        let caret = egui::Rect::from_min_size(egui::pos2(text_left + cc as f32 * char_w, top + cl as f32 * row_h), egui::vec2(2.0, row_h));
        if eb.ensure_visible {
            ui.scroll_to_rect(caret, None); // 커서가 보이도록 스크롤.
            eb.ensure_visible = false;
        }
        // 렌더(보이는 줄만 — first/last는 위에서 계산).
        let sel = eb.selection();
        let text_col = ui.visuals().text_color();
        let sel_col = ui.visuals().selection.bg_fill;
        let painter = ui.painter().clone();
        // 현재 줄 강조(포커스 + 선택 없을 때) — 편집 위치 식별.
        if focused && sel.is_none() && cl >= first && cl < last {
            let ly = top + cl as f32 * row_h;
            let bg = egui::Rect::from_min_size(egui::pos2(left, ly), egui::vec2(content_w, row_h));
            painter.rect_filled(bg, egui::Rounding::ZERO, CURLINE);
        }
        for i in first..last {
            let y = top + i as f32 * row_h;
            if let Some((s, e)) = sel {
                paint_sel(&painter, eb, i, s, e, y, text_left, row_h, char_w, sel_col);
            }
            painter.text(egui::pos2(text_left, y), egui::Align2::LEFT_TOP, eb.line_string(i), mono.clone(), text_col);
            painter.text(egui::pos2(text_left - 6.0, y), egui::Align2::RIGHT_TOP, (i + 1).to_string(), mono.clone(), GUTTER);
        }
        if focused {
            painter.rect_filled(caret, egui::Rounding::ZERO, CARET);
        }
    });
}

/// 한 줄 i의 선택 [s,e) 겹침을 배경으로 칠한다(줄 끝을 넘으면 약간 더 그려 개행 선택 표시).
#[allow(clippy::too_many_arguments)]
fn paint_sel(painter: &egui::Painter, eb: &EditBuf, i: usize, s: usize, e: usize, y: f32, text_left: f32, row_h: f32, char_w: f32, col: egui::Color32) {
    let ls = eb.rope.line_to_char(i);
    let le = ls + eb.line_len(i);
    if e <= ls || s > le {
        return;
    }
    let a = s.max(ls) - ls;
    let b = e.min(le) - ls;
    let mut x1 = text_left + b as f32 * char_w;
    if e > le {
        x1 += char_w * 0.5; // 개행까지 선택됨 표시.
    }
    let x0 = text_left + a as f32 * char_w;
    painter.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1, y + row_h)), egui::Rounding::ZERO, col);
}

/// 포인터 위치 → char 인덱스(등폭 가정).
fn hit(eb: &EditBuf, p: egui::Pos2, top: f32, text_left: f32, row_h: f32, char_w: f32) -> usize {
    let line = (((p.y - top) / row_h).floor().max(0.0) as usize).min(eb.rope.len_lines().saturating_sub(1));
    let col = (((p.x - text_left) / char_w).round().max(0.0) as usize).min(eb.line_len(line));
    eb.rope.line_to_char(line) + col
}
