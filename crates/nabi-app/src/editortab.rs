//! 에디터 도크 탭/분리 창 본문 — 도구막대 + 라인번호 거터 + code 에디터 + 하단 상태바.
//! Ctrl+휠 줌(이 에디터만), Ctrl+S 저장. (E3에서 syntect 하이라이트가 layouter로 붙는다.)

use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const WARN: egui::Color32 = egui::Color32::from_rgb(240, 180, 60);
const GUTTER: egui::Color32 = egui::Color32::from_rgb(120, 130, 145);
const GUTTER_CUR: egui::Color32 = egui::Color32::from_rgb(225, 230, 250); // 커서 줄 번호(밝게).
const GUTTER_SEL: egui::Color32 = egui::Color32::from_rgb(62, 80, 116); // 선택 줄 번호 배경.

/// 한 에디터 문서를 그리고 액션(저장)을 돌려준다. dirty/줌은 doc에 직접 반영.
pub(crate) fn render_editor_tab(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang, recent: &[String]) -> EditorAct {
    if doc.hex.is_some() {
        let act = crate::edithexview::hex_view(ui, doc, lang); // HEX(이진) 편집기(N3).
        doc.dirty = doc.hex.as_ref().map(|h| h.dirty).unwrap_or(false);
        return act;
    }
    if doc.edit.is_some() {
        let act = crate::editbufview::edit_view(ui, doc, lang); // 대용량 rope 편집기(E6).
        doc.dirty = doc.edit.as_ref().map(|e| e.dirty).unwrap_or(false); // 탭 제목 * 동기화.
        return act;
    }
    if doc.big.is_some() {
        return big_view(ui, doc, lang); // 대용량 가상화 뷰어(읽기 전용).
    }
    let mut act = EditorAct::default();
    // nabiPad 메뉴바(표시 설정 시).
    if doc.show_menu { crate::editormenu::menu_bar(ui, doc, lang, &mut act, recent); }
    ui.horizontal(|ui| {
        if !doc.show_menu
            && ui.button("\u{2630}").on_hover_text(tr(lang, "nabipad.menu.show")).clicked()
        {
            act.toggle_menu_bar = true; // 숨김 상태에서 메뉴바 켜기.
        }
        if ui.button(format!("\u{1f4be} {}", tr(lang, "editor.save"))).clicked() { act.save = true; }
        if ui.button(tr(lang, "editor.saveas")).clicked() { act.save_as = true; }
        ui.toggle_value(&mut doc.highlight, "\u{1f3a8}").on_hover_text(tr(lang, "editor.highlight"));
        ui.toggle_value(&mut doc.wrap, "\u{21b5}").on_hover_text(tr(lang, "editor.wrap"));
        ui.toggle_value(&mut doc.show_ws, "\u{00b7}").on_hover_text(tr(lang, "editor.showws"));
        ui.toggle_value(&mut doc.readonly, "\u{1f512}").on_hover_text(tr(lang, "editor.readonly"));
        if doc.dirty { ui.colored_label(WARN, "\u{25cf}"); }
    });
    ui.separator();
    if !doc.loaded {
        ui.horizontal(|ui| { ui.spinner(); ui.label(tr(lang, "sftp.downloading")); });
        return act;
    }
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::renameui::ctrl_wheel_zoom(ui, over); // Ctrl+휠로 이 에디터만 확대/축소.
    if dz != 0.0 { doc.font_size = (doc.font_size + dz).clamp(8.0, 40.0); }
    if over && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) { act.save = true; }
    if over && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) { doc.find.open = !doc.find.open; }
    if doc.find.open { crate::editorfind::find_bar(ui, doc, lang); ui.separator(); }
    // 하단 상태바를 패널로 고정(탭 바깥 스크롤 방지) — 본문이 남은 공간을 정확히 채운다.
    // 커서 Ln/Col은 본문 렌더에서 나오므로 1프레임 지연값(메모리)을 표시.
    let cur_id = ui.id().with("ed_cur");
    let scroll_id = ui.id().with("ed_scroll");
    let last_cur: (usize, usize, usize) = ui.data(|d| d.get_temp(cur_id)).unwrap_or((1, 1, 0));
    // 미니맵(우측 개요) — 직전 프레임 스크롤 상태로 그리고, 클릭 시 목표 오프셋을 받는다.
    let mut mm_target = None;
    if doc.minimap {
        let (oy, ch, vh): (f32, f32, f32) = ui.data(|d| d.get_temp(scroll_id)).unwrap_or((0.0, 1.0, 1.0));
        let mm = egui::SidePanel::right(ui.id().with("ed_mm")).exact_width(84.0).resizable(false);
        mm.show_inside(ui, |ui| mm_target = crate::editorminimap::minimap(ui, &doc.text, oy, ch, vh));
    }
    // 개요(좌측 아웃라인) — 헤더/정의 줄 목록, 클릭 시 그 줄로 점프(작은 파일만 — 매프레임 스캔 비용).
    let mut ol_line = None;
    if doc.outline && doc.text.len() < crate::editorhl::MAX_HL_BYTES {
        let items = crate::editoroutline::outline_items(&doc.text);
        let ol = egui::SidePanel::left(ui.id().with("ed_ol")).default_width(170.0);
        ol.show_inside(ui, |ui| ol_line = crate::editoroutline::outline_panel(ui, &items));
    }
    egui::TopBottomPanel::bottom(ui.id().with("ed_status")).show_inside(ui, |ui| {
        let (enc, eol) = crate::editorstatus::editor_status(ui, doc, last_cur, lang); act.set_encoding = enc; act.set_eol = eol;
    });
    let row_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::monospace(doc.font_size)));
    // 점프(찾기/줄이동/개요)는 대상 줄을 화면 ≈40% 지점에 두어 위아래 맥락이 보이게 한다(VS Code식). 미니맵 스크럽은 정확 위치.
    let vh = ui.available_height();
    let center = |l: usize| (l as f32 * row_h - vh * 0.4).max(0.0);
    let off = mm_target.or(ol_line.map(center)).or(doc.find.scroll_to.take().map(center)); // 미니맵>개요>찾기 우선.
    let area = if doc.wrap { egui::ScrollArea::vertical() } else { egui::ScrollArea::both() };
    let mut sa = area.auto_shrink([false, false]).id_salt("ed_body");
    if let Some(o) = off { sa = sa.vertical_scroll_offset(o); }
    let out = sa.show(ui, |ui| editor_body(ui, doc, lang, &mut act));
    if doc.minimap { ui.data_mut(|d| d.insert_temp(scroll_id, (out.state.offset.y, out.content_size.y, out.inner_rect.height()))); }
    ui.data_mut(|d| d.insert_temp(cur_id, out.inner));
    act
}

/// 대용량 가상화 뷰어: 보이는 줄만 디코드·렌더(읽기 전용) + 라인번호 + 인덱싱 진행.
fn big_view(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang) -> EditorAct {
    let mut act = EditorAct::default();
    // nabiPad 메뉴바(표시 설정 시) — 읽기 전용 뷰어에서도 노출(찾기·메뉴 토글 등).
    if doc.show_menu { crate::editormenu::menu_bar(ui, doc, lang, &mut act, &[]); }
    let over = ui.rect_contains_pointer(ui.max_rect());
    let dz = crate::renameui::ctrl_wheel_zoom(ui, over);
    if dz != 0.0 { doc.font_size = (doc.font_size + dz).clamp(8.0, 40.0); }
    if over && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) { doc.find.open = !doc.find.open; }
    if doc.find.open {
        crate::editorfind::find_bar(ui, doc, lang);
        ui.separator();
    }
    let fsize = doc.font_size;
    let scroll_line = doc.find.scroll_to.take(); // 찾기/줄이동 대상.
    let big = doc.big.as_ref().unwrap();
    let (lc, ready, bytes, enc) = (big.line_count(), big.ready(), big.bytes, big.encoding());
    ui.horizontal(|ui| {
        if !doc.show_menu && ui.button("\u{2630}").on_hover_text(tr(lang, "nabipad.menu.show")).clicked() {
            act.toggle_menu_bar = true;
        }
        ui.colored_label(GUTTER, format!("\u{1f4d6} {}", tr(lang, "editor.bigview")));
        ui.separator();
        ui.label(format!("{lc} lines \u{00b7} {} \u{00b7} {enc}", crate::browserfs::human(bytes as u64)));
        if !ready {
            ui.separator();
            ui.spinner();
        }
        ui.separator();
        ui.label(format!("{}px", fsize as i32));
    });
    ui.separator();
    let mono = egui::FontId::monospace(fsize);
    let row_h = ui.fonts_mut(|f| f.row_height(&mono)).max(1.0);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0')).max(6.0);
    let gutter_w = if doc.show_lineno { char_w * (lc.to_string().len().max(4) as f32) + 12.0 } else { 0.0 };
    let text_col = ui.visuals().text_color();
    let avail_w = ui.available_width(); // 창을 가득 채우도록 콘텐츠 최소 폭 기준.
    let mut sa = egui::ScrollArea::both().auto_shrink([false, false]).id_salt("big_view");
    if let Some(l) = scroll_line {
        sa = sa.vertical_scroll_offset((l as f32 * row_h - ui.available_height() * 0.4).max(0.0)); // 대상 줄 ≈40% 지점(맥락 표시).
    }
    sa.show_viewport(ui, |ui, vp| {
        let first = (vp.top() / row_h).floor().max(0.0) as usize;
        let last = ((vp.bottom() / row_h) as usize + 2).min(lc);
        let lines: Vec<String> = (first..last).map(|i| big.line(i)).collect(); // 한 번만 디코드.
        let maxcols = lines.iter().map(|s| s.chars().count()).max().unwrap_or(0);
        ui.set_width((gutter_w + (maxcols as f32 + 2.0) * char_w).max(avail_w));
        ui.set_height(lc as f32 * row_h);
        let (top, left) = (ui.min_rect().top(), ui.min_rect().left());
        let painter = ui.painter();
        for (k, line) in lines.into_iter().enumerate() {
            let y = top + (first + k) as f32 * row_h;
            painter.text(egui::pos2(left + gutter_w, y), egui::Align2::LEFT_TOP, line, mono.clone(), text_col);
            if doc.show_lineno {
                painter.text(egui::pos2(left + gutter_w - 6.0, y), egui::Align2::RIGHT_TOP, (first + k + 1).to_string(), mono.clone(), GUTTER);
            }
        }
    });
    if !ready { ui.ctx().request_repaint(); } // 인덱싱 진행 중 갱신.
    act
}

/// 라인번호 거터 + TextEdit(+ syntect 하이라이트 layouter). 커서 (Ln, Col, 선택 글자수)를 돌려준다.
fn editor_body(ui: &mut egui::Ui, doc: &mut EditorDoc, lang: Lang, act: &mut EditorAct) -> (usize, usize, usize) {
    let (wrap, readonly, show_ws) = (doc.wrap, doc.readonly, doc.show_ws);
    let mono = egui::FontId::monospace(doc.font_size);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0')).max(6.0);
    let lines = doc.text_stats().1.max(1); // 거터 폭(길이 변화 시에만 재스캔 — D 성능).
    // 거터 폭 = 숫자폭 + 여백. 숫자는 편집창 프레임에서 충분히 떨어뜨려(활성 시 가림 방지).
    let gutter_w = if doc.show_lineno { char_w * (lines.to_string().len().max(3) as f32) + 22.0 } else { 0.0 };
    let body_w = (ui.available_width() - gutter_w).max(80.0); // 본문 TextEdit가 창을 채우도록.
    let mut cur = (1usize, 1usize);
    let mut sel_chars = 0usize;
    // syntect 하이라이트(켜짐 + 임계 이하). 캐시로 변경 시에만 재계산.
    let hl = doc.highlight && doc.text.len() < crate::editorhl::MAX_SYNTAX_BYTES;
    let ext = doc.lang_ext(); // 구문 언어 모드 우선(무제목/오탐 문서도 강조), 없으면 확장자.
    let (fsize, hl_id) = (doc.font_size, ui.id().with("hl").value()); // hl_id=증분 강조 캐시 키.
    // wrap(=doc.wrap)이 꺼져 있으면 layouter의 줄바꿈 폭을 무한대로 둔다 — egui가 넘겨주는 폭은
    // 뷰포트 폭(유한값)이라 그대로 쓰면 wrap=off여도 줄바꿈돼 버린다(가로 스크롤로 표시되도록).
    // 0.34: layouter 인자가 &str → &dyn TextBuffer로 바뀌었다(as_str로 동일 사용).
    let mut layouter = move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_w: f32| {
        let text = buf.as_str();
        let max_w = if wrap { wrap_w } else { f32::INFINITY };
        let mut job = if hl {
            crate::editorhl::highlight(hl_id, text, &ext, fsize)
        } else {
            let color = ui.visuals().text_color();
            egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::monospace(fsize), color, max_w)
        };
        job.wrap.max_width = max_w;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    ui.horizontal_top(|ui| {
        ui.add_space(gutter_w);
        // 래핑 켜짐=가용 폭에 맞춰 채우고 그 폭에서 줄바꿈, 꺼짐=폭 무한(가로 스크롤).
        let te = egui::TextEdit::multiline(&mut doc.text)
            .code_editor()
            .desired_rows(25)
            .interactive(!readonly)
            .desired_width(if wrap { body_w } else { f32::INFINITY })
            .layouter(&mut layouter);
        let out = te.show(ui);
        if out.response.changed() { doc.dirty = true; } crate::editorextra::apply_pending_cursor(ui, out.response.id, doc);
        crate::editorctx::editor_context_menu(&out, doc, lang, readonly, act); // 우클릭 표준 메뉴.
        if show_ws { draw_whitespace(ui, &out.galley, out.galley_pos, &mono); }
        // 커서/선택(블럭)의 galley 행을 구해, 한 번 순회로 논리 줄 번호로 환산.
        let rows = &out.galley.rows;
        // 0.34: rcursor가 사라져 CCursor→행/열은 galley.layout_from_cursor로 구한다.
        let (cur_grow, sel_g) = match &out.cursor_range {
            Some(cr) => {
                let (pr, sr) = (out.galley.layout_from_cursor(cr.primary).row, out.galley.layout_from_cursor(cr.secondary).row);
                let s = (!cr.is_empty()).then(|| (pr.min(sr), pr.max(sr)));
                (pr, s)
            }
            None => (usize::MAX, None),
        };
        let (mut cur_line, mut sel_lo, mut sel_hi) = (usize::MAX, usize::MAX, 0usize);
        let mut k = 1usize;
        for (i, r) in rows.iter().enumerate() {
            if i == cur_grow {
                cur_line = k;
            }
            if let Some((lo, hi)) = sel_g {
                if i == lo {
                    sel_lo = k;
                }
                if i == hi {
                    sel_hi = k;
                }
            }
            if r.ends_with_newline {
                k += 1;
            }
        }
        // 라인번호: 논리 줄 시작 행에만. 커서 줄=밝게, 선택 줄=배경 강조.
        let painter = ui.painter();
        // 현재 줄 강조(A1) — 커서가 있는 시각 행 배경.
        if cur_grow != usize::MAX { crate::editorextra::current_line_bg(painter, &out.galley, out.galley_pos, body_w, cur_grow); }
        // 찾기 일치 전체 강조(찾기 바 열림 + 평문 질의 + 작은 파일).
        if doc.find.open && !doc.find.regex && !doc.find.query.is_empty() && doc.text.len() < crate::editorhl::MAX_HL_BYTES {
            crate::editorextra::highlight_matches(painter, &out.galley, out.galley_pos, &doc.text, &doc.find.query, doc.find.ci);
        }
        let mut n = 1usize;
        for (i, row) in rows.iter().enumerate().filter(|_| doc.show_lineno) {
            if i == 0 || rows[i - 1].ends_with_newline {
                let y = out.galley_pos.y + row.rect().top();
                if sel_g.is_some() && n >= sel_lo && n <= sel_hi {
                    let bg = egui::Rect::from_min_max(egui::pos2(out.galley_pos.x - gutter_w + 2.0, y), egui::pos2(out.galley_pos.x - 6.0, y + row.rect().height()));
                    painter.rect_filled(bg, egui::CornerRadius::ZERO, GUTTER_SEL);
                }
                let col = if n == cur_line { GUTTER_CUR } else { GUTTER };
                painter.text(egui::pos2(out.galley_pos.x - 12.0, y), egui::Align2::RIGHT_TOP, n.to_string(), mono.clone(), col);
                if doc.bookmarks.contains(&(n - 1)) { painter.circle_filled(egui::pos2(out.galley_pos.x - gutter_w + 5.0, y + row.rect().height() * 0.5), 3.0, GUTTER_CUR); } // 북마크 점.
                n += 1;
            }
        }
        if let Some(cr) = out.cursor_range {
            let lc = out.galley.layout_from_cursor(cr.primary);
            cur = (lc.row + 1, lc.column + 1);
            doc.cur_line = lc.row; // 북마크 토글/점프 기준(0기반).
            let (a, b) = (cr.primary.index, cr.secondary.index);
            sel_chars = a.max(b) - a.min(b); // 선택 글자 수(상태바 표시).
            // 괄호 매칭(A1) — 작은 파일에서만(비용 제한).
            if doc.text.len() < crate::editorhl::MAX_HL_BYTES { crate::editorextra::highlight_brackets(ui.painter(), &out.galley, out.galley_pos, char_w, &doc.text, a); }
            // 커서 단어 출현 강조(선택 없음 + 찾기 닫힘 + 작은 파일).
            if sel_chars == 0 && !doc.find.open && doc.text.len() < crate::editorhl::MAX_HL_BYTES { crate::editorextra::highlight_word(ui.painter(), &out.galley, out.galley_pos, &doc.text, a); }
        }
        // 한글 더블클릭 단어 선택 — egui의 ASCII 전용 단어경계를 Unicode 기준으로 덮어쓴다(#6).
        if doc.text.len() < crate::editorhl::MAX_HL_BYTES { crate::editorextra::unicode_word_dblclick(ui, &out, &doc.text); }
    });
    (cur.0, cur.1, sel_chars)
}

/// 공백/탭을 흐린 점·화살표로 오버레이(편집 가이드). 보이는 행만 처리(클립 밖은 건너뜀).
fn draw_whitespace(ui: &egui::Ui, galley: &egui::Galley, origin: egui::Pos2, mono: &egui::FontId) {
    let clip = ui.clip_rect();
    let faint = ui.visuals().weak_text_color();
    let painter = ui.painter();
    for row in &galley.rows {
        let y = origin.y + row.rect().top();
        if y + row.rect().height() < clip.top() || y > clip.bottom() {
            continue; // 화면 밖 행은 그리지 않음(대용량 정상 파일에서 비용 절감).
        }
        for g in &row.glyphs {
            let mark = match g.chr {
                ' ' => "\u{00b7}",
                '\t' => "\u{2192}",
                _ => continue,
            };
            painter.text(origin + g.pos.to_vec2(), egui::Align2::LEFT_TOP, mark, mono.clone(), faint);
        }
    }
}
