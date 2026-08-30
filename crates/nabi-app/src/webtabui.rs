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
    // 탭 이름이 쪽 제목을 따라가게 한다 — 그릴 때만 물으면 되고, 안 보이는 탭은 마지막 이름을 쓴다.
    if let Some(v) = &w.view {
        let t = v.title();
        if !t.is_empty() {
            w.title = t;
        }
    }
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
    // 갈 곳이 있는지 먼저 묻는다 — 단추를 늘 켜 두면 눌러도 아무 일이 없어 고장으로 보인다.
    let (can_b, can_f) = match &tab.view {
        Some(v) => (v.can_back(), v.can_forward()),
        None => (false, false),
    };
    let bar = ui.horizontal(|ui| {
        let tip = |s: &str| nabi_i18n::tr(lang, s);
        if ui.add_enabled(can_b, egui::Button::new("\u{25c0}")).on_hover_text(tip("web.back")).clicked() {
            if let Some(v) = &tab.view { v.back(); }
        }
        if ui.add_enabled(can_f, egui::Button::new("\u{25b6}")).on_hover_text(tip("web.fwd")).clicked() {
            if let Some(v) = &tab.view { v.forward(); }
        }
        // 읽어 오는 중에는 멈춤, 아니면 새로고침 — 같은 자리를 두 뜻으로 쓴다(브라우저 관례).
        let busy = tab.view.as_ref().is_some_and(|v| v.is_loading());
        let (glyph, key) = match busy {
            true => ("\u{2715}", "web.stop"),
            false => ("\u{21bb}", "web.reload"),
        };
        if ui.add_enabled(tab.view.is_some(), egui::Button::new(glyph)).on_hover_text(tip(key)).clicked() {
            if let Some(v) = &tab.view {
                match busy {
                    true => v.stop(),
                    false => v.reload(),
                }
            }
        }
        // 주소 칸은 **지금 보고 있는 곳**을 보여야 한다. 링크를 눌러 옮겨 가도 처음 친
        // 주소가 그대로 남아 있으면, 어디에 있는지 알 수 없고 새로고침이 엉뚱한 곳으로 간다.
        //
        // 다만 사용자가 그 칸을 쓰고 있는 중이면 건드리지 않는다 — 치던 것이 사라지는 것이
        // 가장 짜증스럽다(분리 창의 `bar::set_text` 와 같은 규칙).
        let id = egui::Id::new(("weburl", ui.id()));
        let typing = ui.memory(|m| m.has_focus(id));
        if !typing {
            if let Some(v) = &tab.view {
                let now = v.url();
                if !now.is_empty() && now != tab.url {
                    tab.url = now;
                }
            }
        }
        let edit = egui::TextEdit::singleline(&mut tab.url)
            .id(id)
            .desired_width(ui.available_width() - 96.0);
        if ui.add(edit).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(v) = &tab.view {
                v.go(&tab.url);
            }
        }
        // 확대 배율 — 100% 가 아닐 때만 값을 보여 준다. 늘 보이면 자리만 차지한다.
        if let Some(v) = &tab.view {
            let z = v.zoom();
            if (z - 1.0).abs() > 0.01 {
                ui.label(format!("{:.0}%", z * 100.0)).on_hover_text(tip("web.zoom"));
            }
        }
        ui.menu_button("\u{22ee}", |ui| {
            let has = tab.view.is_some();
            if ui.add_enabled(has, egui::Button::new(tip("web.zoomreset"))).clicked() {
                if let Some(v) = &tab.view { v.set_zoom(1.0); }
                ui.close();
            }
            if ui.add_enabled(has, egui::Button::new(tip("web.devtools"))).clicked() {
                if let Some(v) = &tab.view { v.devtools(); }
                ui.close();
            }
            if ui.add_enabled(has, egui::Button::new(tip("web.savepdf"))).clicked() {
                tab.want_pdf = true;
                ui.close();
            }
        });
    })
    .response
    .rect;

    // 남은 자리가 웹 화면 몫이다.
    let area = egui::Rect::from_min_max(
        egui::pos2(ui.max_rect().min.x, bar.max.y + 2.0),
        ui.max_rect().max,
    );
    ui.allocate_rect(area, egui::Sense::hover());

    // Ctrl+휠로 확대/축소 — 브라우저의 관례다. 웹 화면은 자식 창이라 휠을 우리가 못 받지만,
    // 도구 줄과 그 둘레에서는 받을 수 있다. WebView2 자체도 Ctrl+휠을 처리하므로 여기서는
    // 우리 영역에서 굴렸을 때만 거든다.
    let (ctrl, wheel) = ui.ctx().input(|i| (i.modifiers.command, i.smooth_scroll_delta.y));
    if ctrl && wheel.abs() > 0.5 && ui.rect_contains_pointer(area) {
        if let Some(v) = &tab.view {
            let step = if wheel > 0.0 { 1.1 } else { 1.0 / 1.1 };
            v.set_zoom(v.zoom() * step);
        }
    }

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
    // **둘, 웹 화면 자리 위에 우리가 그린 무언가가 있는가.**
    //
    // 예전에는 "마우스 아래에 무엇이 있는가"를 물었다. 틀린 물음이었다 — 도구 줄의
    // 더보기(⋮)를 누르면 마우스는 **단추 위**(우리 층)에 있고 메뉴는 웹 화면 위에 열린다.
    // 그래서 안 숨겼고 메뉴가 웹 화면 뒤로 들어가 보이지 않았다. 설정 창처럼 마우스와
    // 상관없이 뜨는 것도 같은 이유로 가려졌다(사용자 보고 2026-08-30).
    //
    // 그래서 **자리를 여러 곳 짚어 본다.** 한 곳이라도 우리 층이 아니면 그 위에 무언가가
    // 떠 있는 것이다.
    let busy = ui.ctx().input(|i| i.pointer.any_down()) || covered(ui, area);
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

/// 웹 화면 자리 위에 **우리가 그린 무언가가 덮고 있는가**.
///
/// 메뉴·툴팁·창은 전부 egui 의 층(layer)이다. 자식 창인 웹 화면은 우리 그림보다 늘 위에
/// 오므로, 덮는 것이 있으면 그동안 웹 화면을 숨겨야 그것이 보인다.
///
/// 층이 어디를 차지하는지 직접 물을 길이 없어(`Areas::get` 은 공개가 아니다) **자리를
/// 격자로 짚는다.** 5×5 면 메뉴 하나쯤은 반드시 걸린다. 짚는 일은 찾아보기라 싸다.
fn covered(ui: &egui::Ui, area: egui::Rect) -> bool {
    let (ctx, me) = (ui.ctx(), ui.layer_id());
    const N: usize = 5;
    (0..N * N).any(|i| {
        let t = |k: usize| (k as f32 + 0.5) / N as f32;
        let p = egui::pos2(
            area.min.x + area.width() * t(i % N),
            area.min.y + area.height() * t(i / N),
        );
        ctx.layer_id_at(p).is_some_and(|l| l != me)
    })
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
