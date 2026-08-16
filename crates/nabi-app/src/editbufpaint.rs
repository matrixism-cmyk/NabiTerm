//! rope 편집기의 한 줄 그리기 · 선택 배경 · 클릭 히트테스트.
//!
//! 좌표는 전부 **갤리**에서 얻는다(등폭 가정 폐기). 페인트와 히트테스트가 같은 갤리를 쓰므로
//! 넓은 글자·탭이 섞여도 캐럿이 글자 사이로 어긋나지 않는다.

use crate::editbuf::EditBuf;
use crate::editbufcol::DispLine;
use egui::text::CCursor;
use egui::{Align2, Color32, FontId, Galley, Pos2, Rect, CornerRadius};
use std::sync::Arc;

/// 한 줄을 그릴 때 필요한 값 묶음(인자 폭발 방지).
pub(crate) struct RowCtx<'a> {
    pub painter: &'a egui::Painter,
    pub text_left: f32,
    pub row_h: f32,
    pub text_col: Color32,
    pub sel_col: Color32,
    pub gutter_col: Color32,
    pub font: FontId,
    pub show_lineno: bool,
}

/// 줄의 표시 문자열을 갤리로 만든다(egui 내부 갤리 캐시가 같은 줄을 재사용한다).
pub(crate) fn layout(ui: &egui::Ui, text: &str, font: &FontId, col: Color32) -> Arc<Galley> {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), col))
}

/// 갤리 안에서 표시 char 인덱스의 x 좌표.
pub(crate) fn x_at(g: &Galley, disp: usize) -> f32 {
    g.pos_from_cursor(CCursor::new(disp)).min.x
}

/// 한 줄(본문 + 선택 배경 + 줄 번호)을 그린다.
pub(crate) fn row(ctx: &RowCtx, g: &Arc<Galley>, d: &DispLine, i: usize, y: f32, sel: Option<(usize, usize)>, ls: usize) {
    if let Some((s, e)) = sel {
        paint_sel(ctx, g, d, y, s, e, ls);
    }
    ctx.painter.galley(Pos2::new(ctx.text_left, y), g.clone(), ctx.text_col);
    if ctx.show_lineno {
        let pos = Pos2::new(ctx.text_left - 6.0, y);
        ctx.painter.text(pos, Align2::RIGHT_TOP, (i + 1).to_string(), ctx.font.clone(), ctx.gutter_col);
    }
}

/// 줄과 겹치는 선택 구간 [s,e)(문서 전체 char 기준)을 배경으로 칠한다.
/// `ls`는 이 줄의 시작 char 인덱스. 줄 끝을 넘으면 조금 더 그려 개행이 포함됨을 보인다.
fn paint_sel(ctx: &RowCtx, g: &Galley, d: &DispLine, y: f32, s: usize, e: usize, ls: usize) {
    let le = ls + d.chars();
    if e <= ls || s > le {
        return;
    }
    let x0 = ctx.text_left + x_at(g, d.to_disp(s.max(ls) - ls));
    let mut x1 = ctx.text_left + x_at(g, d.to_disp(e.min(le) - ls));
    if e > le {
        x1 += ctx.row_h * 0.3; // 개행까지 선택됨 표시.
    }
    let r = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + ctx.row_h));
    ctx.painter.rect_filled(r, CornerRadius::ZERO, ctx.sel_col);
}

/// 포인터 위치 → 문서 char 인덱스(갤리 기준 + grapheme 경계로 스냅).
pub(crate) fn hit(ui: &egui::Ui, eb: &EditBuf, p: Pos2, top: f32, text_left: f32, row_h: f32, font: &FontId) -> usize {
    let last = eb.rope.len_lines().saturating_sub(1);
    let line = (((p.y - top) / row_h).floor().max(0.0) as usize).min(last);
    let src = eb.line_string(line);
    let d = DispLine::new(&src, eb.tab);
    let g = layout(ui, &d.text, font, Color32::WHITE);
    let cur = g.cursor_from_pos(egui::vec2(p.x - text_left, row_h * 0.5));
    let off = crate::editbufcol::grapheme_snap(&src, d.to_src(cur.index));
    eb.rope.line_to_char(line) + off
}

#[cfg(test)]
mod tests {
    use super::{x_at, DispLine};

    /// 실제 폰트로 레이아웃한 갤리를 준비한다(GPU 없이 epaint만으로 동작).
    fn laid_out(src: &str) -> (DispLine, std::sync::Arc<egui::Galley>) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let d = DispLine::new(src, 4);
        let font = egui::FontId::monospace(14.0);
        let g = ctx.fonts_mut(|f| f.layout_no_wrap(d.text.clone(), font, egui::Color32::WHITE));
        (d, g)
    }

    #[test]
    fn caret_x_and_hit_test_agree() {
        // 캐럿을 그리는 x로 다시 클릭하면 같은 글자로 돌아와야 한다. 이 왕복이 깨지면
        // 탭·넓은 글자가 있는 줄에서 클릭 위치와 커서가 어긋난다(등폭 가정 시절의 버그).
        for src in ["ab\tcd\te", "plain text", "\t\tindented", "a\tb"] {
            let (d, g) = laid_out(src);
            for i in 0..=d.chars() {
                let x = x_at(&g, d.to_disp(i));
                let cur = g.cursor_from_pos(egui::vec2(x + 0.1, 0.0));
                assert_eq!(d.to_src(cur.index), i, "{src:?}의 {i}번째에서 왕복 실패");
            }
        }
    }

    #[test]
    fn caret_x_is_monotonic() {
        let (d, g) = laid_out("ab\tcd\te");
        let mut prev = f32::NEG_INFINITY;
        for i in 0..=d.chars() {
            let x = x_at(&g, d.to_disp(i));
            assert!(x >= prev, "캐럿 x가 뒤로 갔다({i})");
            prev = x;
        }
    }
}
