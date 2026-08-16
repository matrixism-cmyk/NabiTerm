//! nabiPad 미니맵 — 문서 전체를 우측에 축소 개요로 그리고, 현재 뷰포트를 표시한다.
//! 클릭/드래그하면 그 위치로 스크롤(반환한 목표 오프셋을 본문 ScrollArea가 적용).
//! 줄당이 아니라 미니맵 픽셀 행마다 한 줄을 매핑해 그려, 대용량에서도 비용이 일정하다.

use egui::{Color32, Rect, CornerRadius, Sense};

const BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 40); // 미니맵 배경.
const LINE: Color32 = Color32::from_rgba_premultiplied(170, 180, 200, 90); // 코드 줄.
const VIEW: Color32 = Color32::from_rgba_premultiplied(150, 170, 210, 36); // 현재 뷰포트.

/// 미니맵을 그린다. `off`/`content_h`/`viewport_h`는 직전 프레임의 본문 스크롤 상태.
/// 클릭/드래그 시 그 지점으로 가는 목표 스크롤 오프셋을 Some으로 돌려준다.
pub fn minimap(ui: &mut egui::Ui, text: &str, off: f32, content_h: f32, viewport_h: f32) -> Option<f32> {
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    minimap_by(ui, n, |i| lines.get(i).map(|l| l.trim_end().chars().count()).unwrap_or(0), off, content_h, viewport_h)
}

/// 줄 수 + 줄 길이 클로저 버전 — rope 등 &str이 아닌 저장소도 같은 미니맵을 쓴다(T6-3).
pub fn minimap_by(
    ui: &mut egui::Ui,
    n_lines: usize,
    line_len: impl Fn(usize) -> usize,
    off: f32,
    content_h: f32,
    viewport_h: f32,
) -> Option<f32> {
    let rect = ui.max_rect();
    let (mm_h, mm_w) = (rect.height().max(1.0), rect.width());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, BG);

    // 줄 길이를 가로 막대로(미니맵 픽셀 행 ↔ 문서 줄 매핑 — 행 수 무관 일정 비용).
    let n = n_lines.max(1);
    let rows = mm_h as usize;
    for py in 0..rows {
        let li = py * n / rows.max(1);
        let len = line_len(li).min(100);
        if len > 0 {
            let w = (len as f32 / 100.0) * (mm_w - 8.0);
            let y = rect.top() + py as f32;
            painter.rect_filled(Rect::from_min_size(egui::pos2(rect.left() + 4.0, y), egui::vec2(w, 1.0)), CornerRadius::ZERO, LINE);
        }
    }

    // 현재 뷰포트 박스.
    if content_h > 1.0 {
        let vy = rect.top() + (off / content_h) * mm_h;
        let vh = ((viewport_h / content_h) * mm_h).max(4.0);
        painter.rect_filled(Rect::from_min_size(egui::pos2(rect.left(), vy), egui::vec2(mm_w, vh)), CornerRadius::ZERO, VIEW);
    }

    // 클릭/드래그 → 그 지점을 뷰포트 중앙에 두는 목표 오프셋.
    let resp = ui.interact(rect, ui.id().with("mm_interact"), Sense::click_and_drag());
    if (resp.clicked() || resp.dragged()) && content_h > viewport_h {
        if let Some(p) = resp.interact_pointer_pos() {
            let frac = ((p.y - rect.top()) / mm_h).clamp(0.0, 1.0);
            return Some((frac * content_h - viewport_h / 2.0).clamp(0.0, content_h - viewport_h));
        }
    }
    None
}
