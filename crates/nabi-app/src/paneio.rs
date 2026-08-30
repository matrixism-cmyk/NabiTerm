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
    report_wheel: bool,
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
        let (row, col) = crate::panecell::at(rect, cw, ch, p);
        let (row, col) = (row.min(u16::MAX as usize) as u16, col.min(u16::MAX as usize) as u16);
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
        let wheel = raw_wheel(i);
        let wy = if report_wheel { wheel.y } else { 0.0 };
        if wy != 0.0 {
            let mb = if wy > 0.0 {
                MouseBtn::WheelUp
            } else {
                MouseBtn::WheelDown
            };
            out.extend(mouse_report(sgr, mb, col, row, true, mods));
        }
        let wx = if report_wheel { wheel.x } else { 0.0 };
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

pub(crate) use nabi_editor::uiutil::{consume_wheel, raw_wheel};

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
        egui::CornerRadius::same(3),
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

pub(crate) use nabi_editor::uiutil::clipboard_text;

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
/// 포커스 pane의 입력 이벤트만 복제해 돌려준다(비포커스는 빈 목록).
///
/// 비포커스 pane은 어차피 `typed` 판정에서 걸러지는데도 매 프레임 이벤트 목록을 복제하고
/// 키 인코딩까지 돌던 낭비를 없앤다(성능 리뷰 2026-08-19). 진단 로그도 여기서 한 번에.
pub(crate) fn focused_events(ui: &egui::Ui, is_focused: bool, pane: nabi_types::PaneId) -> Vec<egui::Event> {
    if !is_focused {
        return Vec::new();
    }
    let events = ui.input(|i| i.events.clone());
    trace_input_events(&events, pane); // IME 회귀 진단(nabi_input=trace).
    events
}

/// 포커스 pane에 도달한 키/텍스트/IME 이벤트 진단(`NABI_LOG=nabi_input=trace`).
///
/// 한글 조합 파괴 회귀(2026-08-18, egui 0.36 request_focus→interrupt_ime) 추적에 쓴 계측 —
/// 기본 필터(info)에선 침묵하므로 상시 켜 둔다. IME 문제 보고가 오면 trace로 재현을 받는다.
pub(crate) fn trace_input_events(events: &[egui::Event], pane: nabi_types::PaneId) {
    if events.is_empty() || !tracing::enabled!(target: "nabi_input", tracing::Level::TRACE) {
        return;
    }
    let mut s = String::new();
    for ev in events {
        match ev {
            egui::Event::Key { key, pressed, .. } => s.push_str(&format!("Key({key:?},p={pressed}) ")),
            egui::Event::Text(t) => s.push_str(&format!("Text({t:?}) ")),
            egui::Event::Ime(i) => s.push_str(&format!("Ime({i:?}) ")),
            _ => {}
        }
    }
    if !s.is_empty() {
        tracing::trace!(target: "nabi_input", "pane={pane:?} {s}");
    }
}

pub(crate) fn grab_term_focus(ui: &mut egui::Ui, active: bool) {
    if !active {
        return;
    }
    let sink = term_focus_sink();
    let zero = egui::Rect::from_min_size(ui.min_rect().min, egui::Vec2::ZERO);
    ui.interact(zero, sink, egui::Sense::focusable_noninteractive());
    ui.memory_mut(|m| {
        // egui 0.36부터 request_focus()가 IME 조합을 중단시킨다(Memory::interrupt_ime).
        // 이미 싱크가 포커스인데 매 프레임 재요청하면 한글 초성·중성·종성 조합이
        // 프레임마다 파괴돼 한글 타이핑이 전멸한다(2026-08-18 긴급 버그) — 비었을 때만 잡는다.
        if m.focused().is_none() {
            m.request_focus(sink);
        }
        if m.has_focus(sink) {
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

/// 포인터가 이 pane 위에 있고 **가려지지 않았는가**.
///
/// `rect.contains(p)`만 보면 위에 뜬 창(설정·도움말·분리 창)을 **뚫고** 뒤쪽 터미널이
/// 반응한다 — 설정 창에서 글자를 드래그하면 뒤 터미널이 선택되고, 링크를 누르면 뒤 pane의
/// 링크가 열린다. 포인터 위치의 최상위 레이어가 이 pane의 레이어여야 진짜 이 pane 위다.
pub(crate) fn pointer_on_pane(ui: &egui::Ui, rect: egui::Rect, p: egui::Pos2) -> bool {
    rect.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
}

/// 터미널로 입력을 보낼 수 없는 상태(싱크 외 위젯에 포커스가 있거나 팝업이 열림).
pub(crate) fn term_input_blocked(ctx: &egui::Context) -> bool {
    let sink = term_focus_sink();
    egui::Popup::is_any_open(ctx) || ctx.memory(|m| m.focused().is_some_and(|f| f != sink))
}

/// 우클릭/중간클릭 붙여넣기로 들어온 **원문 텍스트**(바이트로 만들지 않는다).
///
/// 예전에는 여기서 바로 바이트로 바꿔 PTY로 보냈다. 그 바람에 여러 줄 붙여넣기 확인이
/// Ctrl+Shift+V에만 걸리고 **우클릭 붙여넣기는 그냥 통과**했다 — 터미널에서 제일 많이
/// 쓰는 붙여넣기가 안전장치를 비껴간 셈이다. 이제 원문만 돌려주고, 확인 여부는
/// 붙여넣기 한 길(`NabiApp::paste_text_to_pane`)에서 정한다.
pub(crate) fn right_click_paste_text(ui: &egui::Ui, rect: egui::Rect) -> Option<String> {
    // rect_contains_pointer는 레이어 가림을 반영 — 위에 떠 있는 분리 창("창 안에 띄우기")이
    // 가린 경우 뒤쪽 pane이 우클릭을 가로채지 않게 한다(#5).
    let clicked = ui.input(|i| {
        i.pointer.button_clicked(egui::PointerButton::Secondary)
            || i.pointer.button_clicked(egui::PointerButton::Middle)
    }) && ui.rect_contains_pointer(rect);
    if !clicked {
        return None;
    }
    clipboard_text().filter(|t| !t.is_empty())
}

/// 분리 창의 키보드 붙여넣기(Ctrl+V)를 가로챈다 — 여러 줄이면 이벤트를 걷어내고 원문을 준다.
///
/// 메인 창은 `intercept_keyboard_paste`가 같은 일을 하는데, 그건 메인 ctx에서만 돌아서
/// 분리 창에서는 확인 없이 셸로 곧장 들어갔다.
pub(crate) fn take_multiline_paste(ui: &egui::Ui, warn: bool) -> Option<String> {
    if !warn {
        return None;
    }
    let text = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Paste(s) if s.contains(['\n', '\r']) => Some(s.clone()),
            _ => None,
        })
    })?;
    ui.ctx().input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
    Some(text)
}

/// 클립보드 텍스트를 붙여넣기 바이트로 만든다(bracketed면 200~/201~ 래핑,
/// 내부에 끼어든 200~/201~ 마커는 제거해 주입을 막는다).
pub(crate) fn wrap_paste(text: &str, bracketed: bool) -> Vec<u8> {
    // 끼어든 마커 제거 + 제어문자 위생(붙여넣기 주입 방지).
    let clean = nabi_render::sanitize_paste(&text.replace("\x1b[200~", "").replace("\x1b[201~", ""));
    // **줄바꿈을 CR 하나로 맞춘다.** 윈도우에서 복사한 글은 줄마다 CRLF 라서, 그대로
    // 보내면 셸이 엔터를 두 번 받은 것처럼 굴어 빈 줄이 하나씩 더 실행된다. 다른 윈도우
    // 터미널들도 붙여넣기에서 이렇게 맞춘다.
    //
    // 만들어 두고 아무도 쓰지 않던 함수였다(2026-08-30 전수 점검에서 찾았다).
    let clean = nabi_render::paste::normalize_newlines(&clean);
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

    /// 윈도우 글을 붙여넣으면 줄바꿈이 **\r 하나**로 간다.
    ///
    /// 안 맞추면 줄마다 엔터가 두 번 들어가 빈 줄이 그만큼 더 실행된다.
    #[test]
    fn crlf_becomes_one_cr() {
        assert_eq!(wrap_paste("a\r\nb\r\nc", false), b"a\rb\rc".to_vec());
        // \n 만 있는 글도 셸이 기대하는 \r 로 간다.
        assert_eq!(wrap_paste("a\nb", false), b"a\rb".to_vec());
        // 대괄호 붙여넣기에서도 같다 — 감싸는 것과 줄바꿈은 다른 이야기다.
        assert_eq!(wrap_paste("a\r\nb", true), b"\x1b[200~a\rb\x1b[201~".to_vec());
    }

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
