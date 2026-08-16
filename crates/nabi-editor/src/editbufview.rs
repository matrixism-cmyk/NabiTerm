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
pub fn edit_view(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> EditorAct {
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
    ui.separator();
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::uiutil::ctrl_wheel_zoom(ui, over);
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
    let (cur, sel) = match doc.edit.as_ref() {
        Some(e) => (e.cursor_line_col(), e.selected_text().chars().count()),
        None => return act, // edit 없는 문서 — 그릴 본문이 없다(패닉 대신 빈 화면).
    };
    egui::Panel::bottom(ui.id().with("eb_status")).show(ui, |ui| {
        eb_status(ui, doc, cur, sel, lang);
    });
    let m = edit_body(ui, doc, lang);
    // 우클릭 메뉴에서 온 동작은 여기서 처리한다(클로저 안에서 doc을 또 빌릴 수 없다).
    if let Some(t) = m.copy {
        if !t.is_empty() {
            ui.ctx().copy_text(t);
        }
    }
    if m.paste && !doc.readonly {
        if let Some(t) = crate::uiutil::clipboard_text() {
            if let Some(eb) = doc.edit.as_mut() {
                eb.insert(&t.replace("\r\n", "\n"));
            }
        }
    }
    if m.find {
        doc.find.open = true;
    }
    act
}

/// 하단 상태바: Ln/Col · 선택수 · 줄/바이트 · 인코딩 · EOL · 줌.
fn eb_status(ui: &mut egui::Ui, doc: &EditorDoc, cur: (usize, usize), sel: usize, lang: Lang) {
    let Some(eb) = doc.edit.as_ref() else { return };
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("Ln {}, Col {}", cur.0 + 1, cur.1 + 1));
        if sel > 0 {
            ui.separator();
            ui.label(format!("Sel {sel}"));
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

/// 가상화 편집 영역 — 보이는 줄만 그리고, 포커스 시 키/마우스로 편집한다.
fn edit_body(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> crate::editbufmenu::BufMenuAct {
    let (fsize, readonly) = (doc.font_size, doc.readonly);
    let hl_ext = doc.highlight.then(|| doc.lang_ext()); // rope 구문 강조(T6-1).
    let mono = egui::FontId::monospace(fsize);
    let row_h = ui.fonts_mut(|f| f.row_height(&mono)).max(1.0);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0')).max(6.0);
    let scroll_line = doc.find.scroll_to.take();
    let mut menu_act = crate::editbufmenu::BufMenuAct::default();
    let Some(eb) = doc.edit.as_mut() else { return menu_act };
    let lc = eb.rope.len_lines();
    let gutter_w = char_w * (lc.to_string().len().max(4) as f32) + 12.0;
    let avail_w = ui.available_width(); // 창을 가득 채우도록 콘텐츠 최소 폭 기준.
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("eb_body");
    if let Some(l) = scroll_line {
        sa = sa.vertical_scroll_offset(l as f32 * row_h);
    }
    sa.show_viewport(ui, |ui, vp| {
        let first = (vp.top() / row_h).floor().max(0.0) as usize;
        let last = ((vp.bottom() / row_h) as usize + 2).min(lc);
        // 가로 범위는 지금까지 본 최장 줄로 정한다. 보이는 줄만 쓰면 스크롤할 때마다
        // 범위가 늘었다 줄었다 해서 가로 스크롤바가 요동친다.
        let seen = (first..last).map(|i| eb.disp_line(i).width()).max().unwrap_or(0);
        eb.seen_cols = eb.seen_cols.max(seen);
        let content_w = (gutter_w + (eb.seen_cols as f32 + 2.0) * char_w).max(avail_w);
        ui.set_width(content_w);
        ui.set_height(lc as f32 * row_h);
        let (top, left) = (ui.min_rect().top(), ui.min_rect().left());
        let text_left = left + gutter_w;
        // 포커스/입력. 클릭 영역은 보이는 부분 전체.
        let resp = ui.interact(ui.clip_rect(), ui.id().with("eb_area"), egui::Sense::click_and_drag());
        // 우클릭 메뉴 — 대용량 문서에서도 표준 편집 명령을 쓸 수 있게(단축키 전용이 아니라).
        resp.context_menu(|ui| {
            menu_act = crate::editbufmenu::context_menu(ui, eb, lang, readonly);
        });
        if resp.clicked() || resp.drag_started() {
            resp.request_focus();
        }
        let focused = resp.has_focus();
        // 마우스 → 커서/선택. 명시적 제스처(클릭/드래그)에만 반응한다 —
        // 단순 누름-유지 프레임에서 선택을 확장하던 버그(클릭 시 한 줄 역상) 수정.
        // Alt+드래그 = 컬럼(박스) 선택(T6-3). 앵커는 (줄, 표시열)로 프레임 간 유지.
        let box_anchor_id = ui.id().with("box_anchor");
        if let Some(p) = resp.interact_pointer_pos() {
            let pos = crate::editbufpaint::hit(ui, eb, p, top, text_left, row_h, &mono);
            let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
            let line_col = |eb: &EditBuf, pos: usize| {
                let l = eb.rope.char_to_line(pos.min(eb.rope.len_chars()));
                let c = pos - eb.rope.line_to_char(l);
                (l, eb.disp_line(l).to_disp(c.min(eb.line_len(l))))
            };
            if resp.drag_started() {
                if alt {
                    let a = line_col(eb, pos);
                    ui.data_mut(|d| d.insert_temp(box_anchor_id, a));
                    eb.box_select(a, a);
                } else if shift {
                    eb.move_head(pos);
                } else {
                    eb.set_cursor(pos);
                }
                eb.undo_open = false;
            } else if resp.dragged() {
                match ui.data(|d| d.get_temp::<(usize, usize)>(box_anchor_id)) {
                    Some(a) if alt => eb.box_select(a, line_col(eb, pos)),
                    _ => eb.move_to(pos, true), // 드래그 중 선택 확장.
                }
            } else if resp.clicked() {
                if shift {
                    eb.move_to(pos, true); // Shift+클릭 = 현재 위치까지 확장.
                } else {
                    eb.set_cursor(pos); // 단순 클릭 = 커서 이동(선택 해제).
                    eb.undo_open = false;
                }
            }
            if resp.drag_stopped() {
                ui.data_mut(|d| d.remove::<(usize, usize)>(box_anchor_id));
            }
        }
        // 키보드(포커스 + 편집 가능).
        if focused {
            let page = (vp.height() / row_h).max(1.0) as i64;
            crate::editbufkeys::apply_keys(ui, eb, page, readonly);
        }
        // 캐럿 위치도 갤리에서 — 그리는 좌표와 같은 함수를 쓴다(스크롤은 painter 빌림 전에).
        let (cl, cc) = eb.cursor_line_col();
        let cd = eb.disp_line(cl);
        let cg = crate::editbufpaint::layout(ui, &cd.text, &mono, egui::Color32::WHITE);
        let cx = text_left + crate::editbufpaint::x_at(&cg, cd.to_disp(cc));
        let caret = egui::Rect::from_min_size(egui::pos2(cx, top + cl as f32 * row_h), egui::vec2(2.0, row_h));
        if eb.ensure_visible {
            ui.scroll_to_rect(caret, None); // 커서가 보이도록 스크롤.
            eb.ensure_visible = false;
        }
        // 선택 페인트: 박스(멀티범위) 포함 전 범위(캐럿 제외).
        let sel_ranges: Vec<(usize, usize)> = eb
            .sel
            .ranges()
            .iter()
            .filter(|r| !r.is_caret())
            .map(|r| (r.start(), r.end()))
            .collect();
        let sel = eb.selection();
        let painter = ui.painter().clone();
        let ctx = crate::editbufpaint::RowCtx {
            painter: &painter,
            text_left,
            row_h,
            text_col: ui.visuals().text_color(),
            sel_col: ui.visuals().selection.bg_fill,
            gutter_col: GUTTER,
            font: mono.clone(),
            show_lineno: true,
        };
        // 현재 줄 강조(포커스 + 선택 없을 때) — 편집 위치 식별.
        if focused && sel.is_none() && cl >= first && cl < last {
            let ly = top + cl as f32 * row_h;
            let bg = egui::Rect::from_min_size(egui::pos2(left, ly), egui::vec2(content_w, row_h));
            painter.rect_filled(bg, egui::CornerRadius::ZERO, CURLINE);
        }
        // rope 구문 강조: 보이는 창의 스팬만 체크포인트 하이라이터로 계산(T6-1).
        let hl_id = ui.id().with("ropehl").value();
        let win_spans = hl_ext
            .as_deref()
            .and_then(|ext| crate::ropehl::window_spans(hl_id, eb, ext, first, last));
        for i in first..last {
            let d = eb.disp_line(i);
            let g = match win_spans.as_ref().and_then(|w| w.get(i - first)) {
                Some(spans) if !spans.is_empty() => {
                    let src = eb.line_string(i);
                    crate::editbufpaint::layout_spans(ui, &d, &src, spans, &mono, ctx.text_col)
                }
                _ => crate::editbufpaint::layout(ui, &d.text, &mono, ctx.text_col),
            };
            crate::editbufpaint::row(&ctx, &g, &d, i, top + i as f32 * row_h, &sel_ranges, eb.rope.line_to_char(i));
        }
        if focused {
            painter.rect_filled(caret, egui::CornerRadius::ZERO, CARET);
            // 멀티캐럿(박스 타자 중) — 나머지 캐럿도 표시해 어디에 입력될지 보이게.
            if eb.sel.len() > 1 {
                for r in eb.sel.ranges() {
                    let l = eb.rope.char_to_line(r.head.min(eb.rope.len_chars()));
                    let c = r.head - eb.rope.line_to_char(l);
                    let d = eb.disp_line(l);
                    let g2 = crate::editbufpaint::layout(ui, &d.text, &mono, egui::Color32::WHITE);
                    let x = text_left + crate::editbufpaint::x_at(&g2, d.to_disp(c.min(eb.line_len(l))));
                    let rct = egui::Rect::from_min_size(egui::pos2(x, top + l as f32 * row_h), egui::vec2(2.0, row_h));
                    painter.rect_filled(rct, egui::CornerRadius::ZERO, CARET);
                }
            }
        }
    });
    menu_act
}
