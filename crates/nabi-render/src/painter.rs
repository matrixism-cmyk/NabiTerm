//! egui Painter 기반 터미널 그리드 렌더.
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, Ui, Vec2};
use nabi_types::{CellAttrs, Rgba};
use nabi_vt::{CursorShape, RenderCell, TermModel, Theme};

/// 폰트의 셀(너비·높이) 픽셀 크기.
pub fn cell_size(ui: &Ui, font: &FontId) -> (f32, f32) {
    // 0.34: 측정이 지연 셰이핑을 건드릴 수 있어 &mut 뷰가 필요하다(fonts_mut).
    let w = ui.ctx().fonts_mut(|f| f.glyph_width(font, 'M')).max(1.0);
    let h = ui.ctx().fonts_mut(|f| f.row_height(font)).max(1.0);
    (w, h)
}

/// 모델의 화면을 rect 안에 그린다.
///
/// `find`가 있으면 일치 셀을, `sel`(start_row,start_col,end_row,end_col)이 있으면 선택 셀을
/// 하이라이트한다.
pub fn paint(
    ui: &mut Ui,
    rect: Rect,
    font: FontId,
    model: &TermModel,
    theme: &Theme,
    find: Option<&str>,
    keywords: &[String],
    sel: Option<(usize, usize, usize, usize)>,
    rect_sel: bool,
    focused: bool,
    blink_on: bool,
    preedit: &str,
) {
    let painter = ui.painter_at(rect);
    // 글리프 캐시 유효성은 프레임당 1회만 확인한다(셀마다 Context 잠금 잡던 비용 제거).
    crate::glyphcache::begin_frame(ui.ctx());
    let (cw, ch) = cell_size(ui, &font);
    painter.rect_filled(rect, egui::CornerRadius::ZERO, to_c32(theme.bg));
    // 키워드 규칙을 한 번만 파싱(단어 + 색). "단어=#RRGGBB" 형식, 색 생략 시 검색 일치색.
    let rules: Vec<(&str, Rgba)> = keywords
        .iter()
        .filter(|k| !k.is_empty())
        .map(|k| crate::highlight::split_highlight_rule(k, theme.match_color))
        .collect();
    // 렌더 행은 캐시 사용(내용·스크롤·테마 불변 프레임은 재할당 없이 빌려 씀 — 스크롤/유휴 비용↓).
    let rows_v = model.rows_cached(theme);
    let (gen, offset) = (model.render_gen(), model.scrollback_offset());
    // 밑줄(링크+하이퍼링크) 맵도 (gen,offset) 키로 캐시 — 매 프레임 전화면 링크 스캔/복제를 피한다.
    let ul_map = model.underlines_cached(gen, offset, || {
        let wrapped: Vec<bool> = (0..rows_v.len()).map(|r| model.row_wrapped(r as u16)).collect();
        let mut m: Vec<Vec<bool>> = rows_v.iter().map(|row| vec![false; row.len()]).collect();
        // 호버 판정과 **같은 캐시**를 쓴다 — 한 프레임에 화면 링크 스캔은 최대 한 번.
        crate::screenurls::with_screen_urls(
            std::ptr::from_ref(model) as usize,
            gen,
            offset,
            || crate::screenurls::screen_urls(&rows_v, &wrapped),
            |urls| {
                for su in urls {
                    for &(gr, s, e) in &su.segs {
                        // 빈 행이면 `len-1`이 0으로 포화해 `0..=0`이 되고, 그 인덱싱이 패닉한다.
                        // 렌더러 패닉은 UI 스레드를 통째로 죽인다(v0.1.41 사례) — 폭을 먼저 본다.
                        if let Some(mm) = m.get_mut(gr).filter(|mm| s < mm.len()) {
                            let hi = e.min(mm.len() - 1);
                            mm[s..=hi].fill(true);
                        }
                    }
                }
            },
        );
        for (r, hrow) in model.hyperlink_map().into_iter().enumerate() {
            if let Some(mm) = m.get_mut(r) {
                for (c, on) in hrow.into_iter().enumerate() {
                    if on { if let Some(x) = mm.get_mut(c) { *x = true; } }
                }
            }
        }
        m
    });
    // 행 셰이프 버퍼를 프레임당 하나만 쓰고 행마다 drain한다(할당 재사용).
    let mut shapes: Vec<egui::Shape> = Vec::with_capacity(rows_v.first().map_or(0, Vec::len));
    // 검색·키워드 강조 스크래치: 행을 한 번만 평탄화하고 버퍼를 프레임 내내 재사용한다.
    let query = find.filter(|q| !q.is_empty());
    let (mut scan, mut marks) = (crate::findhl::RowScan::default(), Vec::<bool>::new());
    for (r, row) in rows_v.iter().enumerate() {
        // 셀별 강조색(None=강조 없음). 검색 일치는 match_color, 키워드는 규칙 색.
        let mut hl: Vec<Option<Rgba>> = Vec::new();
        if query.is_some() || !rules.is_empty() {
            hl = vec![None; row.len()];
            scan.rebuild(row);
            let mut apply = |pat: &str, color: Rgba, hl: &mut Vec<Option<Rgba>>| {
                marks.clear();
                marks.resize(row.len(), false);
                scan.mark(pat, &mut marks);
                for (i, m) in marks.iter().enumerate() {
                    // 방어적 get_mut — 길이 어긋남 시에도 렌더러 패닉(UI 스레드 즉사) 방지(과거 ul_map 패닉 클래스).
                    if *m {
                        if let Some(slot) = hl.get_mut(i) { *slot = Some(color); }
                    }
                }
            };
            if let Some(q) = query {
                apply(q, theme.match_color, &mut hl);
            }
            for (word, color) in &rules {
                apply(word, *color, &mut hl); // 키워드 규칙이 검색 강조를 덮어쓴다(기존 순서 유지).
            }
        }
        // 밑줄 캐시(cached_underlines, 키=render_gen/offset)와 rows_v(rows_cached, 키=dirty_gen/
        // offset/theme)는 캐시 키가 달라, 새 pane이 빈 모델로 시작했다 채워지는 한 프레임 동안
        // 길이가 어긋날 수 있다. 인덱싱하면 렌더(=UI 스레드)가 패닉→앱 즉사하므로 .get()으로 방어.
        let ul: &[bool] = ul_map.get(r).map_or(&[], Vec::as_slice);
        paint_row(
            &painter, rect, r as f32, cw, ch, &font, row, theme, &hl, ul, sel, rect_sel, &mut shapes,
        );
    }
    // 명령 블록 바(Warp식): 프롬프트 줄 좌측에 종료코드 색 막대(초록=성공/빨강=실패/회색=진행·미상).
    for (row, exit) in model.visible_prompt_rows() {
        let y = rect.top() + f32::from(row) * ch;
        let c = match exit {
            Some(0) => Color32::from_rgb(0x3f, 0xb9, 0x50),
            Some(_) => Color32::from_rgb(0xe5, 0x4b, 0x4b),
            None => Color32::from_rgb(0x70, 0x70, 0x70),
        };
        let bar = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(2.0, ch));
        painter.rect_filled(bar, egui::CornerRadius::ZERO, c);
    }
    // 스크롤백(히스토리)을 보는 중에는 커서를 그리지 않는다.
    if model.scrollback_offset() == 0 {
        paint_cursor(&painter, rect, model, cw, ch, theme, focused, blink_on);
        // IME 조합 중 텍스트(한글 초성·중성·종성 진행)를 커서 위에 밑줄로 오버레이.
        if !preedit.is_empty() {
            crate::overlay::paint_preedit(&painter, rect, model, cw, ch, &font, theme, preedit);
        }
    }
    if focused {
        crate::input::advertise_ime(ui, rect, model, cw, ch); // 한/영·CJK 조합 활성.
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    painter: &Painter,
    rect: Rect,
    r: f32,
    cw: f32,
    ch: f32,
    font: &FontId,
    row: &[RenderCell],
    theme: &Theme,
    highlights: &[Option<Rgba>],
    underlines: &[bool],
    sel: Option<(usize, usize, usize, usize)>,
    rect_sel: bool,
    shapes: &mut Vec<egui::Shape>,
) {
    let y = rect.top() + r * ch;
    let ri = r as usize;
    let adv_of = |cell: &RenderCell| if cell.attrs.contains(CellAttrs::WIDE) { cw * 2.0 } else { cw };
    // 셀 배경색 판정(강조>선택>SGR배경) — 두 패스 공용.
    let fill_of = |i: usize, cell: &RenderCell| -> Option<Color32> {
        if let Some(c) = highlights.get(i).copied().flatten() {
            Some(to_c32(c))
        } else if sel.is_some_and(|(sr, sc, er, ec)| if rect_sel { ri >= sr && ri <= er && i >= sc && i <= ec } else { (ri > sr || (ri == sr && i >= sc)) && (ri < er || (ri == er && i <= ec)) }) {
            Some(to_c32(theme.sel_color))
        } else if cell.bg != theme.bg {
            Some(to_c32(cell.bg))
        } else {
            None
        }
    };
    // 패스 1 — 배경 fill 런 배칭(G2): 연속 동일색을 한 rect로. 텍스트보다 먼저 그려 글자가 위에 온다.
    let mut x = rect.left();
    let (mut fill_clr, mut fill_x): (Option<Color32>, f32) = (None, x);
    for (i, cell) in row.iter().enumerate() {
        let cur = fill_of(i, cell);
        if cur != fill_clr {
            flush_fill(painter, fill_x, x, y, ch, fill_clr);
            (fill_clr, fill_x) = (cur, x);
        }
        x += adv_of(cell);
    }
    flush_fill(painter, fill_x, x, y, ch, fill_clr);
    // 패스 2 — 텍스트(셀별 정확한 그리드 x) + 밑줄. 각 글자를 자기 셀 위치에 그려, 런 배칭의
    // 글자별 픽셀 라운딩 누적 드리프트(글자 겹침/그리드 어긋남)를 원천 차단한다. 공백은 글리프가 없어 생략.
    //
    // 글리프는 캐시된 갤리를 재사용하고(색은 그릴 때 치환), 셰이프는 모아 한 번에 넘긴다 —
    // painter.text()를 셀마다 부르면 호출당 Context 쓰기잠금 2회 + 할당 2회가 붙는다.
    let mut x = rect.left();
    // 링크 밑줄(단일 스타일)은 런으로 모아 한 줄로 그린다(줌서도 처음~끝 정확·이음새 없음). SGR 밑줄은 셀별.
    let (mut ul_clr, mut ul_x): (Option<Color32>, f32) = (None, x);
    for (i, cell) in row.iter().enumerate() {
        let adv = adv_of(cell);
        let fg = if highlights.get(i).copied().flatten().is_some() {
            Color32::BLACK // 강조 셀: 밝은 배경 위 검은 글자.
        } else if cell.attrs.contains(CellAttrs::BOLD) {
            let c = to_c32(cell.fg); // ANSI bold=bright: 전경색을 밝게.
            Color32::from_rgb(c.r().saturating_add(48), c.g().saturating_add(48), c.b().saturating_add(48))
        } else {
            to_c32(cell.fg)
        };
        if !cell.text.is_empty() && cell.text != " " {
            let g = crate::glyphcache::galley(painter.ctx(), &cell.text, font);
            shapes.push(egui::Shape::galley(Pos2::new(x, y), g, fg));
        }
        if cell.attrs.contains(CellAttrs::ANY_UNDERLINE) {
            flush_uline(painter, ul_x, x, y, ch, ul_clr.take()); // SGR 밑줄 전 링크 런 종료.
            let c = cell.ul_color.map(to_c32).unwrap_or(fg);
            crate::underline::paint_underline(painter, x, y, ch, adv, cell.attrs, c);
        } else if underlines.get(i).copied().unwrap_or(false) {
            if ul_clr.is_none() { (ul_clr, ul_x) = (Some(fg), x); } // 링크 밑줄 런 시작.
        } else {
            flush_uline(painter, ul_x, x, y, ch, ul_clr.take()); // 링크 밑줄 런 종료.
        }
        x += adv;
    }
    flush_uline(painter, ul_x, x, y, ch, ul_clr);
    // 행의 글리프를 한 번에 넘긴다 — extend는 그래픽 잠금을 1회만 잡는다(셀당 add 대비).
    // drain이라 버퍼 용량은 남아 다음 행·다음 프레임이 재사용한다(행마다 ~10KB 할당 제거).
    // ⚠️ 반드시 '행 단위'로 넘긴다 — 전체 행을 모아 마지막에 넘기면 아래 행의 선택 배경이
    //    위 행 글자 뒤에 깔리면서 디센더(g·y·함)가 잘려 보인다.
    painter.extend(shapes.drain(..));
}

/// 배칭된 링크 밑줄을 한 줄로 그린다(런 시작~끝). 색 None이면 생략.
fn flush_uline(p: &Painter, x0: f32, x1: f32, y: f32, ch: f32, clr: Option<Color32>) {
    if let Some(c) = clr {
        if x1 > x0 {
            crate::underline::paint_underline(p, x0, y, ch, x1 - x0, CellAttrs(CellAttrs::UNDERLINE), c);
        }
    }
}

/// 배칭된 배경 fill을 한 rect로 그린다(색이 None이면 생략).
fn flush_fill(painter: &Painter, x0: f32, x1: f32, y: f32, ch: f32, clr: Option<Color32>) {
    if let Some(c) = clr {
        if x1 > x0 {
            painter.rect_filled(Rect::from_min_size(Pos2::new(x0, y), Vec2::new(x1 - x0, ch)), egui::CornerRadius::ZERO, c);
        }
    }
}

fn paint_cursor(
    painter: &Painter,
    rect: Rect,
    model: &TermModel,
    cw: f32,
    ch: f32,
    theme: &Theme,
    focused: bool,
    blink_on: bool,
) {
    let cur = model.cursor();
    if !cur.visible {
        return;
    }
    let p = Pos2::new(
        rect.left() + cur.col as f32 * cw,
        rect.top() + cur.row as f32 * ch,
    );
    let r = Rect::from_min_size(p, Vec2::new(cw, ch));
    let c = to_c32(theme.cursor_color.unwrap_or(theme.fg));
    if focused {
        // 포커스: 깜빡임 off 프레임이면 생략, 아니면 모양대로 채운다.
        if !blink_on {
            return;
        }
        match theme.cursor_shape {
            CursorShape::Block => {
                let fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 150);
                painter.rect_filled(r, egui::CornerRadius::ZERO, fill);
            }
            CursorShape::Bar => {
                let bar = Rect::from_min_size(p, Vec2::new(2.0, ch));
                painter.rect_filled(bar, egui::CornerRadius::ZERO, c);
            }
            CursorShape::Underline => {
                let line = Rect::from_min_size(Pos2::new(p.x, p.y + ch - 2.0), Vec2::new(cw, 2.0));
                painter.rect_filled(line, egui::CornerRadius::ZERO, c);
            }
        }
    } else {
        // 비포커스: 깜빡임 없이 윤곽선.
        // Inside: 윤곽선이 셀 밖으로 삐져 이웃 셀을 침범하지 않게.
        painter.rect_stroke(r, egui::CornerRadius::ZERO, Stroke::new(1.0, c), egui::StrokeKind::Inside);
    }
}

fn to_c32(c: Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}
