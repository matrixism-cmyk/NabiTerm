//! Foreground 모달 헬퍼 — 분리 창(Order::Middle) 위로 확실히 띄우는 중앙 팝업.
//! 배경 가림(뒤쪽 클릭 차단) + popup 프레임. 닫기확인·붙여넣기확인 등 공용(z-order 일관화).

use egui::{Align2, Area, Color32, Context, Frame, Id, Order, Sense, Ui};

/// 화면 중앙에 Foreground 레이어 모달을 그린다. 배경을 어둡게 덮어 뒤쪽 입력을 차단한다.
pub(crate) fn foreground_modal<R>(ctx: &Context, id: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let screen = ctx.content_rect();
    Area::new(Id::new(format!("{id}_dim"))).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(96));
        ui.allocate_rect(screen, Sense::click_and_drag());
    });
    Area::new(Id::new(id)).order(Order::Foreground).anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| Frame::popup(ui.style()).show(ui, add).inner)
        .inner
}
