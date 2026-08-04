//! 터미널/세션 pane의 우측 오버레이 스크롤바 — 스크롤백(히스토리)이 있을 때만 표시.
//!
//! 드래그/클릭으로 스크롤 위치 이동. alt 화면(vim/less 등)은 스크롤백이 없어 표시 안 함.

use nabi_types::PaneId;

/// 스크롤바 폭(px). 포인터가 이 우측 영역에 있으면 텍스트 선택을 억제한다.
pub(crate) const SB_W: f32 = 12.0;

/// 포인터가 스크롤바 영역(우측 SB_W) 위에 있는지(선택 억제 판정용).
pub(crate) fn over_scrollbar(rect: egui::Rect, pos: Option<egui::Pos2>) -> bool {
    pos.is_some_and(|p| rect.contains(p) && p.x >= rect.right() - SB_W)
}

/// 우측 스크롤바를 그리고 드래그/클릭 시 모델을 스크롤한다. 스크롤백 없으면 아무것도 안 함.
pub(crate) fn draw(ui: &egui::Ui, rect: egui::Rect, pane: PaneId, model: &mut nabi_vt::TermModel) {
    if model.alt_screen() {
        return;
    }
    let history = model.history_size();
    if history == 0 {
        return; // 위로 볼 영역이 없으면 스크롤바 숨김.
    }
    let rows = model.size().rows() as usize;
    let total = (history + rows).max(1) as f32;
    let offset = model.scrollback_offset();

    let track = egui::Rect::from_min_max(
        egui::pos2(rect.right() - SB_W, rect.top()),
        egui::pos2(rect.right(), rect.bottom()),
    );
    let resp = ui.interact(track, ui.id().with(("term_sb", pane.get())), egui::Sense::click_and_drag());

    // 썸(thumb): 보이는 창이 전체에서 차지하는 위치·비율.
    let top_line = (history - offset) as f32; // 0=맨 위, history=현재 창 top.
    // 트랙이 24px보다 짧으면 min>max로 clamp가 패닉한다(UI 스레드=앱 즉사) → 하한을 트랙 높이로 제한.
    let thumb_h = ((rows as f32 / total) * track.height()).clamp(24.0_f32.min(track.height()), track.height());
    let thumb_top = (track.top() + (top_line / total) * track.height())
        .clamp(track.top(), (track.bottom() - thumb_h).max(track.top()));
    let thumb = egui::Rect::from_min_size(
        egui::pos2(track.left() + 2.0, thumb_top),
        egui::vec2(SB_W - 4.0, thumb_h),
    );

    let painter = ui.painter_at(rect);
    let active = resp.hovered() || resp.dragged();
    if active {
        painter.rect_filled(track, egui::Rounding::same(3.0), egui::Color32::from_black_alpha(60));
    }
    let alpha = if active { 200 } else { 120 };
    painter.rect_filled(thumb, egui::Rounding::same(4.0), egui::Color32::from_white_alpha(alpha));

    // 드래그/클릭 → 클릭 지점을 창 중앙으로 맞춰 스크롤.
    if (resp.dragged() || resp.clicked()) && track.height() > 1.0 {
        if let Some(p) = resp.interact_pointer_pos() {
            let frac = ((p.y - track.top()) / track.height()).clamp(0.0, 1.0);
            let target_top = frac * total - rows as f32 / 2.0;
            let target = (history as f32 - target_top).round().clamp(0.0, history as f32) as i64;
            let delta = target - offset as i64;
            if delta != 0 {
                model.scroll_by(delta as i32);
            }
        }
    }
}
