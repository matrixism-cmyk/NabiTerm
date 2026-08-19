//! 터미널 pane의 URL/하이퍼링크 처리(호버 커서 · Ctrl+클릭 · 열기). tabs/windows 공용.

use nabi_vt::{TermModel, Theme};

/// 클릭/호버 위치의 링크를 돌려준다 — 소프트랩 그룹을 이어 붙여 감지하므로 줄바꿈된
/// URL/경로도 끊기지 않고 전체가 한 링크로 잡힌다.
pub(crate) fn screen_url_at(
    model: &TermModel,
    theme: &Theme,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pos: egui::Pos2,
) -> Option<nabi_render::ScreenUrl> {
    if !rect.contains(pos) {
        return None;
    }
    let row = ((pos.y - rect.top()) / ch).floor().max(0.0) as usize;
    let vcol = ((pos.x - rect.left()) / cw).floor().max(0.0) as usize;
    let col = model.render_index_at(row as u16, vcol); // 시각열→render 인덱스(와이드 보정).
    // 명시적 OSC 8 하이퍼링크가 우선(프로그램이 지정한 링크 — 텍스트 모양과 무관).
    if let Some((s, e, uri)) = model.hyperlink_span_at(row as u16, col) {
        return Some(nabi_render::ScreenUrl { segs: vec![(row, s, e)], url: uri });
    }
    // 캐시된 행을 빌려 쓰고(전체 화면 재구축 금지), 링크 목록도 (모델,세대,스크롤) 캐시에서
    // 가져온다 — 포인터가 pane 위에 있기만 해도 매 프레임 전 화면을 스캔하던 비용 제거.
    let rows = model.rows_cached(theme);
    let key = std::ptr::from_ref(model) as usize;
    nabi_render::with_screen_urls(
        key,
        model.render_gen(),
        model.scrollback_offset(),
        || {
            let wrapped: Vec<bool> = (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
            nabi_render::screen_urls(&rows, &wrapped)
        },
        |urls| urls.iter().find(|su| su.contains(row, col)).cloned(),
    )
}

/// 포인터가 URL 셀 위에 있으면 손가락 커서를 표시한다(하이퍼링크 어포던스).
pub(crate) fn hover_url_cursor(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    model: &TermModel,
    theme: &Theme,
) {
    let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    // 위에 창이 떠 있으면 그 창 위에서도 링크 커서가 떠 뒤 pane이 반응하는 것처럼 보인다.
    if !crate::paneio::pointer_on_pane(ui, rect, pos) {
        return;
    }
    if screen_url_at(model, theme, rect, cw, ch, pos).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// 이번 프레임의 Ctrl+좌클릭 위치(있으면).
pub(crate) fn ctrl_click_pos(ui: &egui::Ui) -> Option<egui::Pos2> {
    ui.input(|i| {
        if i.pointer.primary_clicked() && i.modifiers.command {
            i.pointer.interact_pos()
        } else {
            None
        }
    })
}

/// 클릭 위치의 셀이 URL이면 그 URL 문자열을 돌려준다(소프트랩 포함).
pub(crate) fn url_at(
    model: &TermModel,
    theme: &Theme,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pos: egui::Pos2,
) -> Option<String> {
    screen_url_at(model, theme, rect, cw, ch, pos).map(|su| su.url)
}

/// 이번 프레임 Ctrl+좌클릭한 링크의 raw 문자열(부작용 없음 — 호출측이 분기). 호버 커서는 별도.
pub(crate) fn ctrl_click_url(
    ui: &egui::Ui,
    model: &TermModel,
    theme: &Theme,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
) -> Option<String> {
    let pos = ctrl_click_pos(ui)?;
    // 설정·도움말 창 위에서 Ctrl+클릭했는데 뒤 터미널의 링크가 열리면 안 된다.
    if !crate::paneio::pointer_on_pane(ui, rect, pos) {
        return None;
    }
    url_at(model, theme, rect, cw, ch, pos)
}

/// 링크를 왼쪽 버튼으로 길게 누르면(약 0.45초, 이동 없이) 그 링크의 선택 영역 + URL +
/// 메뉴 표시 위치를 돌려준다. 드래그(이동)는 타이머를 리셋해 텍스트 선택과 충돌하지 않는다.
pub(crate) fn link_longpress(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pane: nabi_types::PaneId,
    model: &TermModel,
    theme: &Theme,
) -> Option<(crate::selection::Sel, String, egui::Pos2)> {
    let lp_id = egui::Id::new(("term_lp", pane.get()));
    let (down, ppos, time) =
        ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos(), i.time));
    // ⚠️ 저장 타입(Option<...>)과 조회 타입을 반드시 일치시킨다 — egui temp는 (Id,TypeId)로
    // 키가 걸려, get_temp가 튜플 타입으로 조회하면 Option<튜플>로 저장된 값을 못 찾아 매 프레임
    // None이 되고 타이머가 영원히 리셋된다(길게 누르기 미발동의 진짜 원인이었음).
    let mut lp: Option<(f64, egui::Pos2, bool)> =
        ui.data(|d| d.get_temp::<Option<(f64, egui::Pos2, bool)>>(lp_id)).flatten();
    let mut result = None;
    match (down, ppos.filter(|p| crate::paneio::pointer_on_pane(ui, rect, *p))) {
        (true, Some(p)) => match lp {
            None => {
                lp = Some((time, p, false));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
            }
            Some((t0, p0, false)) => {
                // 버튼을 가만히 누르고 있으면 egui는 이벤트가 없어 리페인트하지 않는다.
                // → 타이머가 진행되도록 명시적으로 다시 그리게 요청한다(없으면 메뉴가 안 뜸).
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
                if (p - p0).length() > 10.0 {
                    lp = Some((time, p, false)); // 이동=드래그 → 타이머 리셋.
                } else if time - t0 > 0.45 {
                    if let Some(su) = screen_url_at(model, theme, rect, cw, ch, p0) {
                        let (sr, sc, _) = su.segs[0];
                        let (er, _, ec) = *su.segs.last().unwrap();
                        let sel = crate::selection::Sel { pane, ar: sr, ac: sc, hr: er, hc: ec, rect: false };
                        result = Some((sel, su.url, p0));
                    }
                    lp = Some((t0, p0, true)); // 1회만 발동.
                }
            }
            _ => {}
        },
        _ => lp = None, // 버튼 뗌/바깥 → 리셋.
    }
    ui.data_mut(|d| d.insert_temp(lp_id, lp));
    result
}

/// OS 기본 핸들러로 URL/파일을 연다.
pub(crate) fn os_open(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}


