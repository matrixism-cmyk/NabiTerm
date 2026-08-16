//! 범용 egui 입력/클립보드 헬퍼 — nabi-app에서 이관(T5-1). 앱은 재수출로 그대로 쓴다.

/// 프레임의 원시 휠 델타(포인트) — 0.34에서 `raw_scroll_delta` 필드가 사라져
/// MouseWheel 이벤트 합산으로 구한다(단위 환산은 egui 기본 관례를 따름).
pub fn raw_wheel(i: &egui::InputState) -> egui::Vec2 {
    i.events.iter().fold(egui::Vec2::ZERO, |acc, e| match e {
        egui::Event::MouseWheel { unit, delta, .. } => {
            let pts = match unit {
                egui::MouseWheelUnit::Point => *delta,
                egui::MouseWheelUnit::Line => *delta * 40.0, // egui line_scroll_speed 기본치.
                egui::MouseWheelUnit::Page => *delta * 600.0,
            };
            acc + pts
        }
        _ => acc,
    })
}

/// 원시 휠 이벤트를 이 프레임에서 소비한다(스크롤 영역 등 다른 위젯이 또 먹지 않게).
pub fn consume_wheel(i: &mut egui::InputState) {
    i.events.retain(|e| !matches!(e, egui::Event::MouseWheel { .. }));
}

/// 시스템 클립보드 텍스트를 읽는다(우클릭 붙여넣기용). 실패 시 None.
pub fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

pub fn ctrl_wheel_zoom(ui: &egui::Ui, over: bool) -> f32 {
    if !over {
        return 0.0;
    }
    let (wheel, ctrl) = ui.input(|i| (raw_wheel(i).y, i.modifiers.command));
    if ctrl && wheel != 0.0 {
        wheel.signum()
    } else {
        0.0
    }
}

