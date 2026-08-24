//! 용량 무제한 텍스트 편집기(N5)의 가상화 렌더 — 보이는 줄만 그린다. 키 처리는 textkeys.
//!
//! 줄 그리기·선택 배경·거터는 rope 편집기와 **같은 페인터**(editbufpaint)를 쓴다. 그쪽은
//! 갤리와 표시 줄만 받으므로 어떤 저장소가 뒤에 있든 상관하지 않는다 — 옮겨 적을 이유가 없다.
//!
//! 접기·미니맵·구문 강조는 여기에 없다. 그 기능들은 문서 전체를 훑어야 하는데, 이 편집기가
//! 존재하는 이유가 바로 "문서 전체를 훑지 않는다"이다. 수 GB 파일에서는 그 편이 정직하다.

use crate::editbufcol::DispLine;
use crate::editbufpaint::{layout, row, RowCtx};
use crate::editor::{EditorAct, EditorDoc};
use crate::textbuf::TextBuf;
use nabi_i18n::{tr, Lang};

const GUTTER: egui::Color32 = egui::Color32::from_rgb(120, 130, 145);
const CARET: egui::Color32 = egui::Color32::from_rgb(220, 225, 245);
// premultiplied는 RGB ≤ alpha라야 정상 반투명 — 넘으면 가산 혼합돼 줄 배경이 글자를 덮는다.
const CURLINE: egui::Color32 = egui::Color32::from_rgba_premultiplied(22, 22, 22, 22);

/// 탭 폭(표시 칸) — 설정에서 받아 오기 전까지의 기본값.
const TAB: usize = 4;

/// 무제한 편집기 탭 본문.
pub fn huge_view(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> EditorAct {
    let mut act = EditorAct::default();
    if doc.show_menu {
        crate::editormenu::menu_bar(ui, doc, lang, &mut act, &[]);
    }
    crate::textbar::toolbar(ui, doc, lang, &mut act);
    ui.separator();
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::uiutil::ctrl_wheel_zoom(ui, over);
    if dz != 0.0 {
        doc.font_size = (doc.font_size + dz).clamp(8.0, 40.0);
    }
    let Some(tb) = doc.huge.as_ref() else { return act };
    let (line, col, sel, lines) = (tb.caret_line(), tb.caret_col(), tb.has_selection(), tb.data.lines());
    egui::Panel::bottom(ui.id().with("tv_status")).show(ui, |ui| {
        crate::textbar::status(ui, doc, (line, col), sel, lines, lang);
    });
    body(ui, doc);
    refused_notice(ui, doc, lang);
    act
}

/// 못 적는 글자를 거절했으면 잠깐 이유를 보여 준다. 3초 뒤 스스로 사라진다.
fn refused_notice(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) {
    let Some(tb) = doc.huge.as_mut() else { return };
    let Some(at) = tb.refused_at else { return };
    if at.elapsed() > std::time::Duration::from_secs(3) {
        tb.refused_at = None;
        return;
    }
    ui.colored_label(egui::Color32::from_rgb(240, 180, 60), tr(lang, "editor.cantencode"));
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
}

/// 한 줄을 그리기 위해 매 프레임 만드는 값들(인자 폭발 방지).
struct View {
    row_h: f32,
    font: egui::FontId,
    text_left: f32,
    top: f32,
}

/// 선택 구간을 **이 줄 안의 표시 문자** 범위로 옮긴다. 줄 밖이면 None.
///
/// 문서 선택은 바이트 단위인데 페인터는 줄 안 문자 단위를 받는다. 줄 끝을 넘는 선택은
/// 끝을 하나 더 크게 줘서 페인터가 "개행까지 선택됨" 표시를 그리게 한다.
fn sel_in_line(tb: &TextBuf, i: usize, d: &DispLine) -> Option<(usize, usize)> {
    let (s, e) = tb.selection();
    if s == e {
        return None;
    }
    let (ls, le_txt) = tb.data.line_range(i);
    let le_all = tb.data.line_end_with_break(i);
    if e <= ls || s >= le_all {
        return None;
    }
    let to_col = |off: u64| tb.data.decode_len(&tb.data.read(ls, (off.min(le_txt) - ls) as usize));
    let a = if s <= ls { 0 } else { to_col(s) };
    let b = if e >= le_all { d.chars() + 1 } else { to_col(e) };
    Some((a, b))
}

fn body(ui: &mut egui::Ui, doc: &mut EditorDoc) {
    let (fsize, readonly, show_lineno) = (doc.font_size, doc.readonly, doc.show_lineno);
    let font = egui::FontId::monospace(fsize);
    let row_h = ui.fonts_mut(|f| f.row_height(&font)).max(1.0);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, '0')).max(6.0);
    let Some(tb) = doc.huge.as_mut() else { return };
    tb.readonly = readonly;
    let lines = tb.data.lines();
    let gutter_w = char_w * (lines.to_string().len().max(4) as f32) + 12.0;
    let avail_w = ui.available_width();
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("tv_body");
    if let Some(l) = tb.scroll_to.take() {
        sa = sa.vertical_scroll_offset(l as f32 * row_h);
    }
    sa.show_viewport(ui, |ui, vp| {
        let first = (vp.top() / row_h).floor().max(0.0) as usize;
        let last = ((vp.bottom() / row_h) as usize + 2).min(lines);
        // 가로 범위는 지금까지 본 최장 줄로 정한다. 보이는 줄만 쓰면 스크롤할 때마다
        // 범위가 늘었다 줄었다 해서 가로 스크롤바가 요동친다.
        let seen = (first..last).map(|i| DispLine::new(&tb.data.line(i), TAB).width()).max().unwrap_or(0);
        tb.seen_cols = tb.seen_cols.max(seen);
        ui.set_width((gutter_w + (tb.seen_cols as f32 + 2.0) * char_w).max(avail_w));
        ui.set_height(lines as f32 * row_h);
        let (top, left) = (ui.min_rect().top(), ui.min_rect().left());
        let v = View { row_h, font: font.clone(), text_left: left + gutter_w, top };
        let resp = ui.interact(ui.clip_rect(), ui.id().with("tv_area"), egui::Sense::click_and_drag());
        if resp.clicked() || resp.dragged() {
            resp.request_focus();
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                let off = hit(ui, tb, p, &v, lines);
                tb.go(off, resp.dragged() || ui.input(|i| i.modifiers.shift));
            }
        }
        if resp.has_focus() {
            // 넣을 수 없는 글자를 만나면 그 사실을 상태 표시로 알린다 — 조용히 무시하면
            // 사용자에게는 그냥 고장으로 보인다.
            if crate::textkeys::handle(ui, tb, last.saturating_sub(first).max(1)) {
                tb.refused_at = Some(std::time::Instant::now());
            }
            tb.scroll_to_caret_if_needed(first, last);
        }
        paint(ui, tb, &v, first, last, show_lineno, resp.has_focus());
    });
}

/// 포인터 위치 → 문서 바이트 오프셋(갤리 기준).
fn hit(ui: &egui::Ui, tb: &TextBuf, p: egui::Pos2, v: &View, lines: usize) -> u64 {
    let line = (((p.y - v.top) / v.row_h).floor().max(0.0) as usize).min(lines.saturating_sub(1));
    let src = tb.data.line(line);
    let d = DispLine::new(&src, TAB);
    let g = layout(ui, &d.text, &v.font, egui::Color32::WHITE);
    let cur = g.cursor_from_pos(egui::vec2(p.x - v.text_left, v.row_h * 0.5));
    let ch = crate::editbufcol::grapheme_snap(&src, d.to_src(cur.index.0));
    tb.data.offset_of_col(line, ch)
}

fn paint(ui: &egui::Ui, tb: &TextBuf, v: &View, first: usize, last: usize, lineno: bool, focus: bool) {
    let painter = ui.painter_at(ui.clip_rect());
    let vis = ui.visuals();
    let ctx = RowCtx {
        painter: &painter, text_left: v.text_left, row_h: v.row_h,
        text_col: vis.text_color(), sel_col: vis.selection.bg_fill,
        gutter_col: GUTTER, font: v.font.clone(), show_lineno: lineno,
    };
    let cur_line = tb.caret_line();
    for i in first..last {
        let y = v.top + i as f32 * v.row_h;
        let src = tb.data.line(i);
        let d = DispLine::new(&src, TAB);
        if i == cur_line && !tb.has_selection() {
            let r = egui::Rect::from_min_size(
                egui::Pos2::new(ui.min_rect().left(), y),
                egui::vec2(ui.min_rect().width(), v.row_h),
            );
            painter.rect_filled(r, egui::CornerRadius::ZERO, CURLINE);
        }
        let g = layout(ui, &d.text, &v.font, ctx.text_col);
        let sel: Vec<(usize, usize)> = sel_in_line(tb, i, &d).into_iter().collect();
        row(&ctx, &g, &d, i, y, &sel, 0);
        if i == cur_line && focus {
            let x = v.text_left + crate::editbufpaint::x_at(&g, d.to_disp(tb.caret_col()));
            let r = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::vec2(1.5, v.row_h));
            painter.rect_filled(r, egui::CornerRadius::ZERO, CARET);
        }
    }
}
