//! 터미널 pane 포인터 입력 헬퍼(마우스 보고 + 붙여넣기). URL 처리는 paneurl.rs.

/// 이번 프레임의 포인터/휠 이벤트를 터미널 마우스 보고 바이트로 변환한다.
pub(crate) fn mouse_reports(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    sgr: bool,
    wants_release: bool,
    wants_motion: bool,
) -> Vec<u8> {
    use nabi_render::{mouse_report, MouseBtn};
    let mut out = Vec::new();
    ui.input(|i| {
        let Some(p) = i.pointer.interact_pos() else {
            return;
        };
        if !rect.contains(p) {
            return;
        }
        let col = ((p.x - rect.left()) / cw).floor().max(0.0) as u16;
        let row = ((p.y - rect.top()) / ch).floor().max(0.0) as u16;
        let md = i.modifiers;
        let mods = (md.shift as u16) * 4 + (md.alt as u16) * 8 + (md.ctrl as u16) * 16;
        for (btn, mb) in [
            (egui::PointerButton::Primary, MouseBtn::Left),
            (egui::PointerButton::Middle, MouseBtn::Middle),
            (egui::PointerButton::Secondary, MouseBtn::Right),
        ] {
            if i.pointer.button_pressed(btn) {
                out.extend(mouse_report(sgr, mb, col, row, true, mods));
            }
            if wants_release && i.pointer.button_released(btn) {
                out.extend(mouse_report(sgr, mb, col, row, false, mods));
            }
        }
        let wy = i.raw_scroll_delta.y;
        if wy != 0.0 {
            let mb = if wy > 0.0 {
                MouseBtn::WheelUp
            } else {
                MouseBtn::WheelDown
            };
            out.extend(mouse_report(sgr, mb, col, row, true, mods));
        }
        let wx = i.raw_scroll_delta.x;
        if wx != 0.0 {
            let mb = if wx > 0.0 {
                MouseBtn::WheelRight
            } else {
                MouseBtn::WheelLeft
            };
            out.extend(mouse_report(sgr, mb, col, row, true, mods));
        }
        // 드래그 모션(1002/1003): 버튼 누른 채 이동하면 모션 비트(32)와 함께 보고.
        if wants_motion && i.pointer.delta() != egui::Vec2::ZERO {
            let held = if i.pointer.button_down(egui::PointerButton::Primary) {
                Some(MouseBtn::Left)
            } else if i.pointer.button_down(egui::PointerButton::Middle) {
                Some(MouseBtn::Middle)
            } else if i.pointer.button_down(egui::PointerButton::Secondary) {
                Some(MouseBtn::Right)
            } else {
                None
            };
            if let Some(mb) = held {
                out.extend(mouse_report(sgr, mb, col, row, true, mods | 32));
            }
        }
    });
    out
}

/// 스크롤백 키(Shift+PageUp/Down/Home/End)를 소비하고 (delta, to_top, to_bottom)를 돌려준다.
/// PageUp/Down은 `page`줄(한 화면) 단위로 스크롤한다.
pub(crate) fn read_scroll_keys(ui: &egui::Ui, page: i32) -> (i32, bool, bool) {
    let shift = egui::Modifiers {
        shift: true,
        ..egui::Modifiers::NONE
    };
    let step = page.max(1);
    let mut scroll = 0i32;
    if ui.input_mut(|i| i.consume_key(shift, egui::Key::PageUp)) {
        scroll += step;
    }
    if ui.input_mut(|i| i.consume_key(shift, egui::Key::PageDown)) {
        scroll -= step;
    }
    let to_top = ui.input_mut(|i| i.consume_key(shift, egui::Key::Home));
    let to_bottom = ui.input_mut(|i| i.consume_key(shift, egui::Key::End));
    (scroll, to_top, to_bottom)
}

/// 스크롤백을 보고 있을 때 우상단에 "▲ N" 배지를 그린다(클릭하면 맨 아래로 → true).
pub(crate) fn draw_scroll_badge(ui: &egui::Ui, rect: egui::Rect, offset: usize) -> bool {
    if offset == 0 {
        return false;
    }
    let badge = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 108.0, rect.top() + 4.0), // 우측 스크롤바(12px)와 겹치지 않게.
        egui::vec2(88.0, 18.0),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        badge,
        egui::Rounding::same(3.0),
        egui::Color32::from_black_alpha(190),
    );
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        format!("\u{25b2} {offset}"),
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(0xff, 0xd0, 0x55),
    );
    ui.interact(badge, ui.id().with("scroll_badge"), egui::Sense::click())
        .clicked()
}

/// 시스템 클립보드 텍스트를 읽는다(우클릭 붙여넣기용). 실패 시 None.
pub(crate) fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// 터미널 포커스 싱크 id(보이지 않는 포커스 대상 — 모든 키가 PTY로 가게 한다).
fn term_focus_sink() -> egui::Id {
    egui::Id::new("nabi_term_focus_sink")
}

/// 터미널이 활성일 때 싱크에 egui 포커스를 잡고 Tab/화살표/Esc 필터를 건다.
/// egui는 `begin_pass`에서 포커스 가진 위젯의 EventFilter가 그 키를 "원하면" 포커스 이동에
/// 쓰지 않으므로, Tab이 메뉴로 새지 않고 셸 자동완성(\t)으로 전달된다.
///
/// 싱크는 **실제 위젯으로 할당**해야 한다 — egui end_pass의 dead-man 스위치가 그 프레임에
/// 할당되지 않은 포커스 위젯의 포커스를 지우기 때문(영(0) 크기라 레이아웃·마우스에 무영향).
pub(crate) fn grab_term_focus(ui: &mut egui::Ui, active: bool) {
    if !active {
        return;
    }
    let sink = term_focus_sink();
    let zero = egui::Rect::from_min_size(ui.min_rect().min, egui::Vec2::ZERO);
    ui.interact(zero, sink, egui::Sense::focusable_noninteractive());
    ui.memory_mut(|m| {
        if m.focused().is_none_or(|f| f == sink) {
            m.request_focus(sink);
            m.set_focus_lock_filter(
                sink,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        }
    });
}

/// 터미널로 입력을 보낼 수 없는 상태(싱크 외 위젯에 포커스가 있거나 팝업이 열림).
pub(crate) fn term_input_blocked(ctx: &egui::Context) -> bool {
    let sink = term_focus_sink();
    ctx.memory(|m| m.any_popup_open() || m.focused().is_some_and(|f| f != sink))
}

/// 우클릭/중간클릭 붙여넣기 바이트(필요 시 bracketed paste 래핑).
pub(crate) fn right_click_paste(ui: &egui::Ui, rect: egui::Rect, bracketed: bool) -> Option<Vec<u8>> {
    // rect_contains_pointer는 레이어 가림을 반영 — 위에 떠 있는 분리 창("창 안에 띄우기")이
    // 가린 경우 뒤쪽 pane이 우클릭을 가로채지 않게 한다(#5).
    let clicked = ui.input(|i| {
        i.pointer.button_clicked(egui::PointerButton::Secondary)
            || i.pointer.button_clicked(egui::PointerButton::Middle)
    }) && ui.rect_contains_pointer(rect);
    if !clicked {
        return None;
    }
    let text = clipboard_text()?;
    if text.is_empty() {
        return None;
    }
    Some(wrap_paste(&text, bracketed))
}

/// 클립보드 텍스트를 붙여넣기 바이트로 만든다(bracketed면 200~/201~ 래핑,
/// 내부에 끼어든 200~/201~ 마커는 제거해 주입을 막는다).
pub(crate) fn wrap_paste(text: &str, bracketed: bool) -> Vec<u8> {
    // 끼어든 마커 제거 + 제어문자 위생(붙여넣기 주입 방지).
    let clean = nabi_render::sanitize_paste(&text.replace("\x1b[200~", "").replace("\x1b[201~", ""));
    if !bracketed {
        return clean.into_bytes();
    }
    let mut v = b"\x1b[200~".to_vec();
    v.extend_from_slice(clean.as_bytes());
    v.extend_from_slice(b"\x1b[201~");
    v
}

#[cfg(test)]
mod tests {
    use super::wrap_paste;

    #[test]
    fn wrap_paste_strips_inner_markers() {
        assert_eq!(wrap_paste("ab", false), b"ab".to_vec());
        let out = wrap_paste("a\x1b[201~b", true);
        assert_eq!(out, b"\x1b[200~ab\x1b[201~".to_vec());
        // 제어문자(ESC)는 비bracketed에서도 제거된다(붙여넣기 주입 방지).
        assert_eq!(wrap_paste("a\x1b[31mb", false), b"a[31mb".to_vec());
    }
}

// URL/하이퍼링크 클릭 처리(hover_url_cursor·ctrl_click_pos·url_at·os_open·
// ctrl_click_ssh·open_url_at)는 paneurl.rs로 분리됨.
