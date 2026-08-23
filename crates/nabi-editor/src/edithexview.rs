//! nabiPad HEX 편집기 렌더 + 입력. 오프셋 │ 16바이트 HEX │ ASCII 거터를 가상 스크롤로 그린다.
//! HEX 칼럼은 16진수, ASCII 칼럼은 인쇄 가능 문자 입력으로 바이트를 덮어쓴다(Tab으로 칼럼 전환).

use crate::edithex::{HexBuf, COLS};
use crate::edithexedit::{parse_hex, to_hex_string};
use crate::editor::{EditorAct, EditorDoc};
use egui::{Color32, Event, Key};
use nabi_i18n::{tr, Lang};

const WARN: Color32 = Color32::from_rgb(240, 180, 60);
const GUTTER: Color32 = Color32::from_rgb(120, 130, 145);
const CUR_ON: Color32 = Color32::from_rgb(70, 95, 150); // 포커스 칼럼 커서.
const CUR_OFF: Color32 = Color32::from_rgba_premultiplied(120, 130, 160, 60); // 반대 칼럼.
const SEL: Color32 = Color32::from_rgba_premultiplied(80, 110, 170, 90); // 블록 선택 배경.

/// HEX 칼럼에서 i번째 바이트의 문자 오프셋(8바이트 뒤에 한 칸 더 띈다).
fn hex_col(i: usize) -> f32 {
    (i * 3 + usize::from(i >= 8)) as f32
}

/// HEX 편집 도크 탭/창 본문(도구막대 + 상태바 + 가상화 편집 영역).
pub fn hex_view(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> EditorAct {
    let mut act = EditorAct::default();
    // nabiPad 메뉴바(표시 설정 시) — HEX 모드에서도 동일하게 노출.
    if doc.show_menu {
        crate::editormenu::menu_bar(ui, doc, lang, &mut act, &[]);
    }
    ui.horizontal(|ui| {
        if !doc.show_menu && ui.button("\u{2630}").on_hover_text(tr(lang, "nabipad.menu.show")).clicked() { act.toggle_menu_bar = true; }
        if ui.button(format!("\u{1f4be} {}", tr(lang, "editor.save"))).clicked() { act.save = true; }
        if ui.button(tr(lang, "editor.saveas")).clicked() { act.save_as = true; }
        if ui.button("\u{1f520} A").on_hover_text(tr(lang, "nabipad.textmode")).clicked() { act.toggle_hex = true; }
        ui.toggle_value(&mut doc.readonly, "\u{1f512}").on_hover_text(tr(lang, "editor.readonly"));
        // 오프셋으로 이동(@입력 → Enter/→).
        if let Some(h) = doc.hex.as_mut() {
            ui.separator();
            ui.label("@");
            let r = ui.add(egui::TextEdit::singleline(&mut h.goto).desired_width(80.0).hint_text("offset"));
            let go = ui.button("\u{2192}").on_hover_text(tr(lang, "nabipad.gotooffset")).clicked();
            if go || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) { h.jump_to_offset(); }
        }
        if doc.hex.as_ref().is_some_and(|h| h.dirty) {
            ui.colored_label(WARN, "\u{25cf}");
        }
    });
    ui.separator();
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::uiutil::ctrl_wheel_zoom(ui, over);
    if dz != 0.0 { doc.font_size = (doc.font_size + dz).clamp(8.0, 40.0); }
    // 바이트 검색 바(Ctrl+F 토글).
    // 바이트 검색 바(Ctrl+F 토글).
    if over && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) { doc.find.open = !doc.find.open; }
    if doc.find.open { if let Some(h) = doc.hex.as_mut() { crate::edithexfind::find_bar(ui, h, doc.readonly, lang); } ui.separator(); }
    let (cur, len, ins, seln, insp) = doc
        .hex
        .as_ref()
        .map(|h| (h.cursor, h.len(), h.insert_mode, h.selection().map(|(a, b)| b - a).unwrap_or(0), h.inspector()))
        .unwrap_or((0, 0, false, 0, None));
    egui::Panel::bottom(ui.id().with("hx_status")).show(ui, |ui| {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(format!("0x{cur:08X}"));
            // 데이터 인스펙터(LE): 간략은 u8/u16/u32, 호버에 부호정수·16진·2진(HxD식).
            if let Some((brief, detail)) = &insp { ui.separator(); ui.label(brief).on_hover_text(detail); }
            ui.separator();
            ui.label(crate::humanfmt::human(len as u64));
            if seln > 0 { ui.separator(); ui.label(format!("Sel {seln}")); }
            ui.separator();
            ui.label(tr(lang, if ins { "nabipad.hexins" } else { "nabipad.hexovr" }));
            ui.separator();
            ui.label(format!("{}px", doc.font_size as i32));
            ui.separator(); ui.label(tr(lang, "nabipad.hexmode"));
        });
    });
    hex_body(ui, doc, lang);
    act
}

/// 가상화 HEX 영역 — 보이는 줄만 그리고, 포커스 시 키/마우스로 편집한다.
fn hex_body(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) {
    let (fsize, readonly) = (doc.font_size, doc.readonly);
    let mono = egui::FontId::monospace(fsize);
    let row_h = ui.fonts_mut(|f| f.row_height(&mono)).max(1.0);
    let cw = ui.fonts_mut(|f| f.glyph_width(&mono, '0')).max(6.0);
    let Some(h) = doc.hex.as_mut() else { return };
    let rows = h.rows();
    let scroll_off = h.scroll_to.take().map(|r| r as f32 * row_h); // 오프셋 이동 스크롤.
    // 칼럼 기준 x(문자 단위): 오프셋8 + 공백2 → HEX, HEX(49) + " | "(3) → ASCII.
    let x_hex = 10.0 * cw;
    let x_ascii = x_hex + 52.0 * cw;
    let content_w = x_ascii + COLS as f32 * cw + cw;
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("hx_body");
    if let Some(o) = scroll_off { sa = sa.vertical_scroll_offset(o); }
    sa.show_viewport(ui, |ui, vp| {
        ui.set_width(content_w.max(ui.available_width()));
        ui.set_height(rows as f32 * row_h);
        let (top, left) = (ui.min_rect().top(), ui.min_rect().left());
        let id = ui.id().with("hx_area");
        let resp = ui.interact(ui.clip_rect(), id, egui::Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() { resp.request_focus(); }
        // 우클릭 컨텍스트 메뉴(복사/잘라내기/삭제/붙여넣기-삽입/바이트 삽입/전체 선택).
        resp.context_menu(|ui| crate::edithexmenu::context_menu(ui, h, readonly, lang));
        // 마우스 → 커서/블록 선택(드래그=선택 확장, Shift+클릭=확장, 클릭=해제).
        if let Some(p) = resp.interact_pointer_pos() {
            let idx = hit_index(h, p, top, left, x_hex, x_ascii, row_h, cw);
            if resp.drag_started() {
                h.anchor = Some(idx);
                h.cursor = idx;
            } else if resp.dragged() {
                h.cursor = idx;
            } else if resp.clicked() {
                if ui.input(|i| i.modifiers.shift) {
                    if h.anchor.is_none() {
                        h.anchor = Some(h.cursor);
                    }
                } else {
                    h.anchor = None;
                }
                h.cursor = idx;
            }
            h.low_nibble = false;
        }
        let focused = resp.has_focus();
        if focused {
            let page = (vp.height() / row_h).max(1.0) as isize;
            apply_keys(ui, h, page, readonly);
        }
        let first = (vp.top() / row_h).floor().max(0.0) as usize;
        let last = ((vp.bottom() / row_h) as usize + 2).min(rows);
        let painter = ui.painter().clone();
        let text_col = ui.visuals().text_color();
        // 블록 선택 배경(보이는 바이트만).
        if let Some((lo, hi)) = h.selection() {
            paint_selection(&painter, lo, hi, first, last, top, left, x_hex, x_ascii, row_h, cw);
        }
        // 커서 강조(포커스 칼럼은 진하게, 반대 칼럼은 옅게).
        if focused { paint_cursor(&painter, h, top, left, x_hex, x_ascii, row_h, cw); }
        // 셀(바이트) 단위로 그린다 — 각 HEX 쌍/ASCII 칸을 cw 격자에 직접 올려, 선택·커서
        // 사각형과 같은 격자에 정렬한다(폰트가 완전 등폭이 아니어도 밑줄/블록이 안 어긋남).
        for r in first..last {
            let y = top + r as f32 * row_h;
            let base = r * COLS;
            painter.text(egui::pos2(left, y), egui::Align2::LEFT_TOP, format!("{base:08X}"), mono.clone(), GUTTER);
            for i in 0..COLS {
                let Some(b) = h.at(base + i) else { break };
                let hx = left + x_hex + hex_col(i) * cw;
                painter.text(egui::pos2(hx, y), egui::Align2::LEFT_TOP, format!("{b:02X}"), mono.clone(), text_col);
                let ax = left + x_ascii + i as f32 * cw;
                let ch = if (0x20..0x7F).contains(&b) { b as char } else { '.' };
                painter.text(egui::pos2(ax, y), egui::Align2::LEFT_TOP, ch, mono.clone(), text_col);
            }
        }
    });
}

/// 커서 위치의 HEX 쌍·ASCII 칸을 강조한다.
#[allow(clippy::too_many_arguments)]
fn paint_cursor(painter: &egui::Painter, h: &HexBuf, top: f32, left: f32, x_hex: f32, x_ascii: f32, row_h: f32, cw: f32) {
    let (r, c) = (h.cursor / COLS, h.cursor % COLS);
    let y = top + r as f32 * row_h;
    let hx = left + x_hex + hex_col(c) * cw;
    let ax = left + x_ascii + c as f32 * cw;
    let (hcol, acol) = if h.ascii { (CUR_OFF, CUR_ON) } else { (CUR_ON, CUR_OFF) };
    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(hx, y), egui::vec2(cw * 2.0, row_h)), egui::CornerRadius::ZERO, hcol);
    painter.rect_filled(egui::Rect::from_min_size(egui::pos2(ax, y), egui::vec2(cw, row_h)), egui::CornerRadius::ZERO, acol);
}

/// 보이는 선택 바이트 [lo,hi)의 HEX·ASCII 칸 배경을 칠한다.
#[allow(clippy::too_many_arguments)]
fn paint_selection(painter: &egui::Painter, lo: usize, hi: usize, first: usize, last: usize, top: f32, left: f32, x_hex: f32, x_ascii: f32, row_h: f32, cw: f32) {
    for idx in lo..hi {
        let (r, c) = (idx / COLS, idx % COLS);
        if r < first || r >= last {
            continue;
        }
        let y = top + r as f32 * row_h;
        // 다음 바이트도 같은 줄(8바이트 경계·줄끝 제외)에서 선택되면 HEX는 사이 공백까지
        // 이어 칠해 연속 블록으로 보이게 한다(ASCII는 칸이 붙어 있어 그대로 연속).
        let bridge = idx + 1 < hi && c != 7 && c != COLS - 1;
        let hw = if bridge { cw * 3.0 } else { cw * 2.0 };
        let hx = left + x_hex + hex_col(c) * cw;
        let ax = left + x_ascii + c as f32 * cw;
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(hx, y), egui::vec2(hw, row_h)), egui::CornerRadius::ZERO, SEL);
        painter.rect_filled(egui::Rect::from_min_size(egui::pos2(ax, y), egui::vec2(cw, row_h)), egui::CornerRadius::ZERO, SEL);
    }
}

/// 포인터 → 바이트 인덱스(칼럼 포커스 ascii도 갱신).
#[allow(clippy::too_many_arguments)]
fn hit_index(h: &mut HexBuf, p: egui::Pos2, top: f32, left: f32, x_hex: f32, x_ascii: f32, row_h: f32, cw: f32) -> usize {
    let r = (((p.y - top) / row_h).floor().max(0.0) as usize).min(h.rows().saturating_sub(1));
    let rel = p.x - left;
    let c = if rel >= x_ascii {
        h.ascii = true;
        ((rel - x_ascii) / cw) as usize
    } else {
        h.ascii = false;
        // HEX 영역: 문자 오프셋을 바이트 인덱스로 환산(8바이트 뒤 한 칸 보정).
        let ch = ((rel - x_hex) / cw).max(0.0) as usize;
        (if ch >= 24 { ch - 1 } else { ch }) / 3
    };
    (r * COLS + c.min(COLS - 1)).min(h.len().saturating_sub(1))
}

/// 키/텍스트 입력을 HEX 버퍼에 적용한다(읽기 전용이면 편집은 건너뜀). 복사/잘라내기/붙여넣기 지원.
fn apply_keys(ui: &egui::Ui, h: &mut HexBuf, page: isize, readonly: bool) {
    let mut copy: Option<String> = None;
    for ev in ui.input(|i| i.events.clone()) {
        match ev {
            Event::Text(t) if !readonly => {
                for c in t.chars() {
                    if h.ascii {
                        if ('\u{20}'..'\u{7f}').contains(&c) {
                            h.input_ascii(c as u8);
                        }
                    } else if let Some(d) = c.to_digit(16) {
                        h.input_nibble(d as u8);
                    }
                }
            }
            Event::Copy => copy = Some(to_hex_string(&h.selected_bytes())),
            Event::Cut => {
                copy = Some(to_hex_string(&h.selected_bytes()));
                if !readonly {
                    h.delete_forward(); // 선택 삭제.
                }
            }
            Event::Paste(s) if !readonly => h.insert_bytes(&parse_hex(&s)),
            Event::Key { key, pressed: true, modifiers, .. } => key_press(h, key, modifiers, page, readonly),
            _ => {}
        }
    }
    if let Some(t) = copy {
        if !t.is_empty() {
            ui.ctx().copy_text(t);
        }
    }
}

/// 단일 키 처리. Shift=선택 확장, Insert=삽입/덮어쓰기 토글, Ctrl+A=전체선택.
fn key_press(h: &mut HexBuf, key: Key, m: egui::Modifiers, page: isize, readonly: bool) {
    if m.command {
        match key {
            Key::A => h.select_all(),
            Key::Z if !readonly && !m.shift => h.undo(),
            Key::Z if !readonly && m.shift => h.redo(),
            Key::Y if !readonly => h.redo(),
            _ => {} // 복사/잘라내기/붙여넣기는 Event로 처리(중복 방지).
        }
        return;
    }
    let sel = m.shift;
    match key {
        Key::ArrowLeft => h.move_by(-1, sel),
        Key::ArrowRight => h.move_by(1, sel),
        Key::ArrowUp => h.move_by(-(COLS as isize), sel),
        Key::ArrowDown => h.move_by(COLS as isize, sel),
        Key::Home => h.line_edge(false, sel),
        Key::End => h.line_edge(true, sel),
        Key::PageUp => h.move_by(-page * COLS as isize, sel),
        Key::PageDown => h.move_by(page * COLS as isize, sel),
        Key::Tab => {
            h.ascii = !h.ascii;
            h.low_nibble = false;
        }
        Key::Insert => h.insert_mode = !h.insert_mode,
        Key::Delete if !readonly => h.delete_forward(),
        Key::Backspace if !readonly => h.backspace(),
        _ => {}
    }
}
