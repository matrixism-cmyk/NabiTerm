//! 영문 팁 한글 오버레이 — 감지된 팁 줄 위에 번역을 덧그린다(터미널 그리드는 불변).
//!
//! 감지·사전은 tiptrans.rs, 선택적 AI 번역은 tipai.rs. 화면 스캔은 내용이 바뀐
//! 프레임에서만 한다(render_gen 캐시) — 매 프레임 전 행을 문자열로 만들면 비싸다.

use nabi_types::PaneId;

/// pane에서 찾은 팁 한 건(캐시 항목).
#[derive(Clone)]
pub(crate) struct TipHit {
    /// 이 결과를 만든 화면 세대(내용이 바뀌면 다시 스캔).
    pub gen: u64,
    /// 화면에서의 행(0=맨 위).
    pub row: u16,
    /// 원문(호버 시 표시).
    pub en: String,
    /// 번역(사전 또는 AI). 없으면 오버레이를 그리지 않는다.
    pub ko: Option<String>,
}

/// 팁 오버레이가 필요로 하는 상태(탭·분리 창 공용).
pub(crate) struct TipState<'a> {
    pub enabled: bool,
    pub ai_on: bool,
    pub cache: &'a mut std::collections::HashMap<PaneId, TipHit>,
    pub ai: &'a mut crate::tipai::TipAi,
}

/// 팁 줄을 찾아 번역을 덧그린다. 번역이 없으면 아무 것도 하지 않는다(원문 그대로 보임).
/// 탭과 분리 창이 같은 코드를 쓴다(표면 드리프트 방지).
pub(crate) fn draw_tip_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    ch: f32,
    font: &egui::FontId,
    bg: nabi_types::Rgba,
    pane: PaneId,
    model: &nabi_vt::TermModel,
    st: &mut TipState,
) {
    if !st.enabled {
        return;
    }
    let gen = model.render_gen();
    if st.cache.get(&pane).is_none_or(|h| h.gen != gen) {
        match scan(model, gen, st.ai_on, st.ai) {
            Some(h) => st.cache.insert(pane, h),
            None => st.cache.remove(&pane),
        };
    }
    // AI 번역이 뒤늦게 도착하면 그 pane의 캐시에 채워 넣는다(다음 프레임부터 표시).
    if let Some(h) = st.cache.get_mut(&pane) {
        if h.ko.is_none() {
            h.ko = st.ai.get(&h.en).map(str::to_owned);
        }
    }
    let Some(hit) = st.cache.get(&pane) else { return };
    let Some(ko) = hit.ko.as_deref() else { return };
    let y = rect.top() + f32::from(hit.row) * ch;
    let row_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), ch));
    let p = ui.painter_at(rect);
    p.rect_filled(row_rect, egui::CornerRadius::ZERO, egui::Color32::from_rgb(bg.r, bg.g, bg.b));
    p.text(
        egui::pos2(rect.left() + 1.0, y),
        egui::Align2::LEFT_TOP,
        format!("\u{1f4ac} {ko}"),
        font.clone(),
        crate::theme_ui::ACCENT,
    );
    // 원문은 호버로 — "번역이 틀렸나?" 싶을 때 바로 확인할 수 있어야 한다.
    ui.interact(row_rect, ui.id().with(("tiptrans", pane.get())), egui::Sense::hover())
        .on_hover_text(&hit.en);
}

impl crate::tabs::TermTabViewer<'_> {
    /// 탭 pane의 팁 오버레이(공통 구현 위임).
    pub(crate) fn tip_overlay(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        ch: f32,
        font: &egui::FontId,
        pane: PaneId,
        model: &nabi_vt::TermModel,
    ) {
        let mut st = TipState {
            enabled: self.tip_overlay,
            ai_on: self.tip_ai_on,
            cache: self.tip_cache,
            ai: self.tip_ai,
        };
        draw_tip_overlay(ui, rect, ch, font, self.theme.bg, pane, model, &mut st);
    }
}

/// 화면 하단에서 마지막 팁 줄을 찾아 번역을 붙인다(내용 변경 시에만 호출).
///
/// 팁·안내는 입력창 근처(화면 아래)에 뜬다 — 전 화면을 문자열로 만들지 않고 하단 몇 줄만
/// 본다(프레임 비용 최소화, 성능 리뷰 원칙 유지).
fn scan(
    model: &nabi_vt::TermModel,
    gen: u64,
    ai_on: bool,
    ai: &mut crate::tipai::TipAi,
) -> Option<TipHit> {
    let rows = model.size().rows() as usize;
    // 화면 전체를 본다 — 팁은 TUI에선 아래쪽이지만 셸 출력에선 위쪽에도 남는다.
    // 내용이 바뀐 프레임에만 오는 경로라 문자열 한 번 만드는 비용은 감당할 만하다.
    let text = model.visible_text(rows);
    // 아래에서 위로 — 가장 최근에 출력된 팁을 고른다.
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate().rev() {
        let r = u16::try_from(i).unwrap_or(0);
        // ② 접두사 없는 안내 줄 — 사전에 확실히 있는 문장만(AI 호출 없음).
        if crate::tiptrans::tip_body(line).is_none() {
            if let Some(ko) = crate::tiptrans::lookup_line(line) {
                return Some(TipHit { gen, row: r, en: (*line).trim().to_owned(), ko: Some(ko.to_owned()) });
            }
            continue;
        }
        let body = crate::tiptrans::tip_body(line).unwrap_or(line);
        let mut ko = crate::tiptrans::lookup(body).map(str::to_owned);
        if ko.is_none() {
            ko = ai.get(body).map(str::to_owned);
            if ko.is_none() && ai_on {
                ai.request(body); // 사전에 없을 때만, 켜져 있을 때만 호출한다.
            }
        }
        return Some(TipHit { gen, row: r, en: body.to_owned(), ko });
    }
    None
}
