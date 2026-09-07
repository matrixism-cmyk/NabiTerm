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

/// 명령이 시작된 자리(성공/진행 중).
const M_PROMPT: egui::Color32 = egui::Color32::from_rgb(110, 150, 200);
/// **실패한** 명령 — 막대만 봐도 어디서 틀어졌는지 보이게 눈에 띄는 색으로.
const M_FAIL: egui::Color32 = egui::Color32::from_rgb(220, 90, 90);
/// 사용자가 손으로 남긴 표식.
const M_USER: egui::Color32 = egui::Color32::from_rgb(230, 200, 90);

/// 우측 스크롤바를 그리고 드래그/클릭 시 모델을 스크롤한다. 스크롤백 없으면 아무것도 안 함.
///
/// `user_marks` 는 사용자가 남긴 절대 줄 번호들([`crate::scrollmark`]). 탭과 분리 창이
/// **같은 것을 넘겨야 한다** — 한쪽에만 빈 배열을 넘기면 그 창에서만 표식이 사라진다.
pub(crate) fn draw(
    ui: &egui::Ui,
    rect: egui::Rect,
    pane: PaneId,
    model: &mut nabi_vt::TermModel,
    user_marks: &[usize],
) {
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
        painter.rect_filled(track, egui::CornerRadius::same(3), egui::Color32::from_black_alpha(60));
    }
    // 표식을 **썸보다 먼저** 그린다. 지금 보는 자리는 썸이 덮어도 되지만, 표식이 썸을
    // 덮으면 어디를 보고 있는지가 가려진다.
    draw_markers(&painter, track, model, user_marks, total as usize);
    // 눈금을 가리키고 있으면 그 명령이 어떻게 끝났는지 붙여 준다.
    let resp = match marker_tip(&resp, track, model, total as usize) {
        Some(t) => resp.on_hover_ui(|ui| { ui.monospace(t); }),
        None => resp,
    };

    let alpha = if active { 200 } else { 120 };
    painter.rect_filled(thumb, egui::CornerRadius::same(4), egui::Color32::from_white_alpha(alpha));

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

/// 트랙 위에 명령 블록·실패·사용자 표식을 눈금으로 그린다.
///
/// 그리는 순서가 곧 우선순위다. 실패를 맨 나중에 그려 다른 눈금에 가리지 않게 한다 —
/// 사람이 이 막대에서 가장 찾고 싶은 것이 그것이기 때문이다.
fn draw_markers(
    painter: &egui::Painter,
    track: egui::Rect,
    model: &nabi_vt::TermModel,
    user_marks: &[usize],
    total: usize,
) {
    let h = track.height();
    let mut ok = Vec::new();
    let mut fail = Vec::new();
    for m in model.prompt_marks() {
        let abs = m.abs.max(0) as usize;
        match m.exit {
            Some(c) if c != 0 => fail.push(abs),
            _ => ok.push(abs),
        }
    }
    let layers: [(&[usize], egui::Color32, bool); 3] = [
        (&ok, M_PROMPT, false),
        (user_marks, M_USER, false),
        (&fail, M_FAIL, true),
    ];
    for (lines, color, wide) in layers {
        for y in crate::scrollbarmark::marker_rows(lines, total, h) {
            // 실패는 트랙을 가로지르고, 나머지는 왼쪽 절반만 — 겹쳐도 서로 읽힌다.
            let x0 = track.left() + if wide { 0.0 } else { 1.0 };
            let w = if wide { track.width() } else { track.width() * 0.5 };
            let r = egui::Rect::from_min_size(
                egui::pos2(x0, track.top() + y as f32),
                egui::vec2(w, 2.0),
            );
            painter.rect_filled(r, egui::CornerRadius::ZERO, color);
        }
    }
}

/// 눈금 위에 마우스를 올리면 **그 명령이 어떻게 끝났는지** 알려 준다.
///
/// 눈금만으로는 "여기서 뭔가 있었다"까지밖에 모른다. 되짚을 자리를 고르려면 종료 코드와
/// 걸린 시간이 필요한데, 그것을 보려고 스크롤해 내려가면 눈금을 본 뜻이 없어진다.
///
/// 글은 번역하지 않는다 — 기호와 숫자뿐이라 어느 말로 봐도 같다.
fn marker_tip(
    resp: &egui::Response,
    track: egui::Rect,
    model: &nabi_vt::TermModel,
    total: usize,
) -> Option<String> {
    let pos = resp.hover_pos()?;
    let y = pos.y - track.top();
    let marks = model.prompt_marks();
    let rows: Vec<u32> = marks
        .iter()
        .map(|m| crate::scrollbarmark::mark_frac(m.abs.max(0) as usize, total))
        .map(|f| f.map(|f| (f * track.height()) as u32).unwrap_or(u32::MAX))
        .collect();
    let i = crate::scrollbarmark::nearest_row(&rows, y, 4.0)?;
    let m = marks[i];
    let mark = match m.exit {
        None => "\u{25cf}".to_string(),           // 아직 도는 중.
        Some(0) => "\u{2713} 0".to_string(),      // 잘 끝났다.
        Some(c) => format!("\u{2717} {c}"),       // 실패.
    };
    let took = match m.ms {
        Some(ms) if ms >= 1000 => format!("  {:.1}s", ms as f64 / 1000.0),
        Some(ms) => format!("  {ms}ms"),
        None => String::new(),
    };
    Some(format!("{mark}{took}"))
}
