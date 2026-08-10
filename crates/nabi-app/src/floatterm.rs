//! 분리 창(별도 OS 창)·"창 안에 띄우기"(인윈도우 오버레이) 공용 터미널 본문 렌더(windows.rs에서 분리).

use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_orchestrator::SharedPanes;
use nabi_proto::Command;
use nabi_types::{GridSize, PaneId};
use nabi_vt::Theme;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 터미널 pane 본문을 주어진 ui에 그린다(OS 창=CentralPanel·인윈도우 오버레이=egui::Window 공용).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_floating_term(
    ui: &mut egui::Ui,
    panes: &SharedPanes,
    cmd_tx: &Sender<Command>,
    grids: &Arc<Mutex<HashMap<PaneId, GridSize>>>,
    pane: PaneId,
    font_size: f32,
    theme: &Theme,
    broadcast: bool,
    find: Option<&str>,
    blink_on: bool,
    link: &mut Option<(String, egui::Pos2)>,
    zoom: &mut Option<(PaneId, f32)>,
    link_click: &mut Option<(PaneId, String)>,
    // paste_req: 붙여넣기 요청(pane, 원문) — 분리 창도 메인과 같은 확인을 거치게 app으로 넘긴다.
    // warn_paste: 여러 줄 붙여넣기 확인 설정(꺼져 있으면 가로채지 않는다).
    paste_req: &mut Option<(PaneId, String)>,
    warn_paste: bool,
) {
    let font = egui::FontId::monospace(font_size);
    let (cw, ch) = nabi_render::cell_size(ui, &font);
    // 가용 영역을 실제로 allocate해야 egui::Window(창 안에 띄우기)가 콘텐츠 0으로 무너지지 않는다.
    // CentralPanel(분리 OS 창)에서는 전체 영역이라 동작 동일.
    let area = ui.available_rect_before_wrap();
    let _ = ui.allocate_rect(area, egui::Sense::hover());
    let rect = area.shrink(4.0);
    let grid = GridSize::new(
        (rect.width() / cw).floor().max(1.0) as u16,
        (rect.height() / ch).floor().max(1.0) as u16,
    );
    let changed = grids
        .lock()
        .map(|mut g| {
            if g.get(&pane) != Some(&grid) {
                g.insert(pane, grid);
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
    if changed {
        let _ = cmd_tx.send(Command::Resize { pane, size: grid });
    }

    let shift = egui::Modifiers { shift: true, ..egui::Modifiers::NONE };
    let mut scroll = 0i32;
    if ui.input_mut(|i| i.consume_key(shift, egui::Key::PageUp)) {
        scroll += 10;
    }
    if ui.input_mut(|i| i.consume_key(shift, egui::Key::PageDown)) {
        scroll -= 10;
    }

    // 여러 줄 Ctrl+V는 바이트로 바뀌기 **전에** 걷어내 확인 경로로 보낸다.
    // 이 처리가 메인 ctx에만 있어서, 분리 창에서는 확인 없이 셸로 곧장 들어갔다.
    if let Some(t) = crate::paneio::take_multiline_paste(ui, warn_paste) {
        *paste_req = Some((pane, t));
    }
    let events = ui.input(|i| i.events.clone());
    let (app_cursor, bracketed, mouse_on, mouse_release, mouse_sgr, mouse_motion) = panes
        .read()
        .ok()
        .and_then(|m| m.get(&pane).cloned())
        .and_then(|v| {
            v.model.lock().ok().map(|md| {
                (
                    md.app_cursor(),
                    md.bracketed_paste(),
                    md.mouse_on(),
                    md.mouse_wants_release(),
                    md.mouse_sgr(),
                    md.mouse_wants_motion(),
                )
            })
        })
        .unwrap_or((false, false, false, false, false, false));
    let composing = events
        .iter()
        .any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Preedit(s)) if !s.is_empty()));
    let bytes = nabi_render::events_to_bytes(&events, app_cursor, bracketed, composing);
    // 이 분리 창의 터미널이 입력 대상 — 포커스 싱크를 잡아 Tab/화살표/Esc가 PTY로 가게 한다.
    crate::paneio::grab_term_focus(ui, true);
    let typed = !bytes.is_empty() && !crate::paneio::term_input_blocked(ui.ctx());
    if typed {
        // 분리 창은 자기 pane만 보여준다 — 브로드캐스트라도 보이지 않는 pane에 쓰지 않는다.
        let cmd = if broadcast {
            Command::Broadcast { panes: vec![pane], data: Bytes::from(bytes) }
        } else {
            Command::WriteInput { pane, data: Bytes::from(bytes) }
        };
        let _ = cmd_tx.send(cmd);
    }

    let over = ui.rect_contains_pointer(rect);
    let (ctrl, shift, wheel) = ui.input(|i| {
        (i.modifiers.command, i.modifiers.shift, i.raw_scroll_delta.y)
    });
    let ctrl_wheel = over && ctrl && wheel != 0.0;
    let shift_wheel = over && shift && !ctrl && wheel != 0.0;
    if mouse_on {
        let rep = crate::paneio::mouse_reports(
            ui, rect, cw, ch, mouse_sgr, mouse_release, mouse_motion, shift_wheel,
        );
        if !rep.is_empty() {
            let _ = cmd_tx.send(Command::WriteInput { pane, data: Bytes::from(rep) });
        }
    }
    if ctrl_wheel {
        *zoom = Some((pane, wheel.signum())); // Ctrl+휠=이 창만 확대/축소(뒤 창으로 안 샘).
    } else if over && wheel != 0.0 && !shift_wheel {
        scroll += (wheel / ch).round() as i32;
    }

    // 우클릭 붙여넣기(앱 마우스 모드가 아닐 때).
    if !mouse_on {
        if let Some(t) = crate::paneio::right_click_paste_text(ui, rect) {
            *paste_req = Some((pane, t)); // 곧장 보내지 않는다(확인 경로).
        }
    }

    let view = panes.read().ok().and_then(|m| m.get(&pane).cloned());
    if let Some(v) = view {
        if let Ok(mut model) = v.model.lock() {
            if typed {
                model.scroll_to_bottom();
            } else if scroll != 0 {
                model.scroll_by(scroll);
            }
            let focused = ui.ctx().input(|i| i.focused);
            nabi_render::paint(ui, rect, font, &model, theme, find, &[], None, false, focused, blink_on, "");
            if crate::paneio::draw_scroll_badge(ui, rect, model.scrollback_offset()) {
                model.scroll_to_bottom();
            }
            if broadcast {
                ui.painter_at(rect).rect_stroke(
                    rect.shrink(1.0),
                    egui::Rounding::ZERO,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(0xff, 0x8c, 0x00)),
                );
            }
            crate::paneurl::hover_url_cursor(ui, rect, cw, ch, &model, theme);
            if let Some(url) = crate::paneurl::ctrl_click_url(ui, &model, theme, rect, cw, ch) {
                *link_click = Some((pane, url)); // ssh·파일참조·URL 모두 caller(open_term_link)가 분기.
            }
            // 링크 길게 누르기 → 복사/열기 팝업(메인 창과 동일, P2).
            if let Some((_, url, lpos)) =
                crate::paneurl::link_longpress(ui, rect, cw, ch, pane, &model, theme)
            {
                *link = Some((url, lpos));
            }
        }
    }
}
