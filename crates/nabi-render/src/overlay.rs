//! 터미널 위에 겹쳐 그리는 오버레이(IME 조합 텍스트 등).

use egui::{Align2, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use nabi_types::Rgba;
use nabi_vt::{TermModel, Theme};

fn to_c32(c: Rgba) -> egui::Color32 {
    egui::Color32::from_rgb(c.r, c.g, c.b)
}

/// IME 조합 문자열을 커서 셀부터 가로로 그린다(강조 배경 + 밑줄 — 미확정 표시).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_preedit(
    painter: &Painter,
    rect: Rect,
    model: &TermModel,
    cw: f32,
    ch: f32,
    font: &FontId,
    theme: &Theme,
    preedit: &str,
) {
    let cur = model.cursor();
    let x0 = rect.left() + cur.col as f32 * cw;
    let y = rect.top() + cur.row as f32 * ch;
    // CJK는 보통 2셀 폭 — 글자별 폭을 추정해 배경/밑줄 길이를 맞춘다.
    let width: f32 = preedit
        .chars()
        .map(|c| if (c as u32) > 0x2e7f { cw * 2.0 } else { cw })
        .sum::<f32>()
        .max(cw);
    let bg = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width, ch));
    painter.rect_filled(bg, egui::Rounding::ZERO, to_c32(theme.sel_color));
    painter.text(Pos2::new(x0, y), Align2::LEFT_TOP, preedit, font.clone(), to_c32(theme.fg));
    let uy = y + ch - 1.0;
    painter.line_segment(
        [Pos2::new(x0, uy), Pos2::new(x0 + width, uy)],
        Stroke::new(1.5, to_c32(theme.fg)),
    );
}
