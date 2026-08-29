//! 웹 탭을 화면에 그린다(배치 AZ).
//!
//! ## 우리가 그리는 것과 그리지 않는 것
//!
//! 페이지 자체는 우리가 그리지 않는다 — 운영체제가 그 자리에 그려 준다. 우리는 **위쪽
//! 도구 줄만** 그리고, 나머지 자리를 웹 화면에게 넘긴다.
//!
//! 도구 줄은 egui 로 그린다. 별도 창은 윈도우 단추를 썼지만 여기서는 그럴 이유가 없다 —
//! 우리 화면 안이니 우리 모양으로 그리는 편이 잘 어울리고, 글꼴 문제도 없다.

use nabi_types::PaneId;

/// 이 pane 이 웹 탭이면 그린다. 그렸으면 true.
///
/// 탭을 그리는 곳에서 갈라 뒀다 — 줄 한도 때문이기도 하지만, **웹 탭만의 규칙**(이번
/// 프레임에 그려졌다고 표시하기·배율 곱하기)이 한자리에 모여 읽기도 낫다.
pub(crate) fn draw_if_web(
    ui: &mut egui::Ui,
    pane: PaneId,
    tabs: &mut std::collections::HashMap<PaneId, crate::webtab::WebTab>,
    seen: &mut std::collections::HashSet<PaneId>,
    hwnd: Option<isize>,
    lang: nabi_i18n::Lang,
) -> bool {
    let Some(w) = tabs.get_mut(&pane) else {
        return false;
    };
    // 그렸다고 표시한다 — 표시하지 않은 웹 탭은 중앙에서 숨긴다(자식 창이라 필요하다).
    seen.insert(pane);
    let ppp = ui.ctx().pixels_per_point();
    render(ui, w, hwnd, ppp, lang);
    true
}

/// 웹 탭 한 칸을 그린다. 자리를 받아 웹 화면을 그 아래에 놓는다.
pub(crate) fn render(
    ui: &mut egui::Ui,
    tab: &mut crate::webtab::WebTab,
    hwnd: Option<isize>,
    ppp: f32,
    lang: nabi_i18n::Lang,
) {
    let bar = ui.horizontal(|ui| {
        // 단추는 화면이 만들어진 뒤에만 뜻이 있다.
        let on = tab.view.is_some();
        if ui.add_enabled(on, egui::Button::new("\u{25c0}")).on_hover_text(nabi_i18n::tr(lang, "web.back")).clicked() {
            if let Some(v) = &tab.view { v.back(); }
        }
        if ui.add_enabled(on, egui::Button::new("\u{25b6}")).on_hover_text(nabi_i18n::tr(lang, "web.fwd")).clicked() {
            if let Some(v) = &tab.view { v.forward(); }
        }
        if ui.add_enabled(on, egui::Button::new("\u{21bb}")).on_hover_text(nabi_i18n::tr(lang, "web.reload")).clicked() {
            if let Some(v) = &tab.view { v.reload(); }
        }
        let edit = egui::TextEdit::singleline(&mut tab.url).desired_width(ui.available_width());
        if ui.add(edit).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(v) = &tab.view {
                v.go(&tab.url);
            }
        }
    })
    .response
    .rect;

    // 남은 자리가 웹 화면 몫이다.
    let area = egui::Rect::from_min_max(
        egui::pos2(ui.max_rect().min.x, bar.max.y + 2.0),
        ui.max_rect().max,
    );
    ui.allocate_rect(area, egui::Sense::hover());

    // 탭을 끌거나 경계선을 옮기는 중이면 **웹 화면을 잠깐 숨긴다.**
    //
    // 자식 창은 우리 그림보다 늘 위에 온다. 그래서 끄는 동안 나오는 안내와 놓을 자리가
    // 웹 화면에 가려 아무것도 안 보인다(사용자 보고 2026-08-29).
    //
    // 두 가지를 본다.
    //
    // **하나, 마우스를 누르고 있는가.** egui 가 눌림을 봤다면 우리 쪽을 만지는 중이다 —
    // 웹 화면 위를 누르면 그 신호는 자식 창으로 가서 egui 는 보지 못한다. 탭 끌기·경계선.
    //
    // **둘, 팝업이 열려 있는가.** 메뉴는 마우스를 뗀 뒤에도 열려 있다. 눌림만 보면
    // 오른쪽 클릭 메뉴가 웹 화면 아래로 들어가 고를 수 없다(사용자 보고 2026-08-29).
    // **셋, 우리가 그린 무언가 위에 마우스가 있는가.** 메뉴·툴팁·창은 전부 egui 의
    // "영역"이다. 그 위에 마우스가 있으면 사용자가 그것을 보고 있는 중이다.
    let busy = ui.ctx().input(|i| i.pointer.any_down()) || something_above(ui);
    if busy {
        if let Some(v) = &mut tab.view {
            v.show(false);
        }
        return;
    }
    if let Some(msg) = &tab.failed {
        // 만들지 못했으면 그 자리에 이유를 적는다. 빈 탭만 두면 고장 난 줄 안다.
        ui.painter().text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            msg,
            egui::FontId::proportional(14.0),
            ui.visuals().error_fg_color,
        );
        return;
    }
    place(tab, hwnd, area, ppp);
}

/// 마우스가 있는 자리에 **우리가 그린 무언가가 위에 있는가**.
///
/// 메뉴·툴팁·창은 전부 egui 의 층(layer)이다. 마우스 아래의 맨 위 층이 이 탭의 층이
/// 아니라면, 그 위에 무언가가 떠 있다는 뜻이다 — 그러면 웹 화면을 숨겨야 그것이 보인다.
fn something_above(ui: &egui::Ui) -> bool {
    let ctx = ui.ctx();
    ctx.pointer_latest_pos()
        .and_then(|p| ctx.layer_id_at(p))
        .is_some_and(|l| l != ui.layer_id())
}

/// 웹 화면을 이 자리에 놓는다. 없으면 만든다.
fn place(tab: &mut crate::webtab::WebTab, hwnd: Option<isize>, area: egui::Rect, ppp: f32) {
    let Some(h) = hwnd else { return };
    if tab.view.is_none() {
        // 처음 그려질 때 만든다 — 탭을 열 때는 아직 자리를 모른다.
        match nabi_web::embed::Embedded::create(h, &tab.url) {
            Ok(v) => tab.view = Some(v),
            Err(e) => {
                tab.failed = Some(e);
                return;
            }
        }
    }
    let Some(v) = &mut tab.view else { return };
    // egui 자리는 논리 점이다. 창 안의 실제 점으로 바꿔 넘긴다 — 배율을 빠뜨리면
    // 배율을 올린 PC 에서 엉뚱한 자리에 놓인다(화면 캡처에서 겪은 것과 같다).
    v.place(
        (area.min.x * ppp).round() as i32,
        (area.min.y * ppp).round() as i32,
        (area.width() * ppp).round() as i32,
        (area.height() * ppp).round() as i32,
    );
    v.show(true);
}

/// 이번 프레임에 그려진 웹 탭들 — 나머지는 숨긴다.
pub(crate) fn seen_default() -> std::collections::HashSet<PaneId> {
    std::collections::HashSet::new()
}
