//! 셀 밑줄 렌더(단일/이중/점선/파선/물결 undercurl). painter에서 분리(라인 한도).

use egui::{Color32, Painter, Pos2, Stroke};
use nabi_types::CellAttrs;

/// 밑줄을 스타일대로 그린다. `color`는 SGR 58 밑줄 색(없으면 호출측이 전경색을 넘긴다).
pub(crate) fn paint_underline(
    p: &Painter,
    x: f32,
    y: f32,
    ch: f32,
    adv: f32,
    attrs: CellAttrs,
    color: Color32,
) {
    let base = y + ch - 1.5;
    let st = Stroke::new(1.0, color);
    if attrs.contains(CellAttrs::UNDERCURL) {
        curl(p, x, base, adv, color);
    } else if attrs.contains(CellAttrs::DOUBLE_UNDERLINE) {
        p.line_segment([Pos2::new(x, base - 1.5), Pos2::new(x + adv, base - 1.5)], st);
        p.line_segment([Pos2::new(x, base + 1.0), Pos2::new(x + adv, base + 1.0)], st);
    } else if attrs.contains(CellAttrs::DOTTED_UNDERLINE) {
        dashes(p, x, base, adv, color, 2.0);
    } else if attrs.contains(CellAttrs::DASHED_UNDERLINE) {
        dashes(p, x, base, adv, color, 4.0);
    } else {
        p.line_segment([Pos2::new(x, base), Pos2::new(x + adv, base)], st);
    }
}

/// 물결(undercurl) — adv를 4구간으로 나눠 위·아래 지그재그로 근사.
fn curl(p: &Painter, x: f32, y: f32, adv: f32, color: Color32) {
    let st = Stroke::new(1.0, color);
    let n = 4;
    let mut prev = Pos2::new(x, y);
    for k in 1..=n {
        let px = x + adv * (k as f32 / n as f32);
        let py = y + if k % 2 == 0 { 1.5 } else { -1.5 };
        p.line_segment([prev, Pos2::new(px, py)], st);
        prev = Pos2::new(px, py);
    }
}

/// 점선/파선 — `step` 간격마다 절반 길이 선분. (점선=짧게, 파선=길게.)
fn dashes(p: &Painter, x: f32, y: f32, adv: f32, color: Color32, step: f32) {
    let st = Stroke::new(1.0, color);
    let mut dx = 0.0;
    while dx < adv {
        let seg = (step * 0.5).min(adv - dx);
        p.line_segment([Pos2::new(x + dx, y), Pos2::new(x + dx + seg, y)], st);
        dx += step;
    }
}
