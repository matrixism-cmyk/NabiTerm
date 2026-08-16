//! 환경설정 모양 섹션의 라이브 미리보기 — settingsui에서 분리(라인 한도).

use nabi_types::Rgba;

/// 현재 색/글꼴 설정으로 그린 터미널 샘플 한 줄(즉시 시각 확인).
/// 오버라이드가 없으면 선택된 테마의 실제 색을 기본값으로 → 테마 변경도 미리보기에 반영한다.
pub(crate) fn appearance_preview(ui: &mut egui::Ui, a: &nabi_config::Appearance) {
    let base = nabi_vt::Theme::preset(&a.theme);
    let col = |s: &str, d: Rgba| {
        let c = Rgba::from_hex(s).unwrap_or(d);
        egui::Color32::from_rgb(c.r, c.g, c.b)
    };
    let bg = col(&a.bg_color, base.bg);
    let fg = col(&a.fg_color, base.fg);
    let sel = col(&a.selection_color, base.sel_color);
    let mat = col(&a.match_color, base.match_color);
    let cur = col(&a.cursor_color, base.cursor_color.unwrap_or(base.fg));
    let sz = a.font_size;
    egui::Frame::NONE.fill(bg).inner_margin(8.0).corner_radius(4).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let t = |s: &str| egui::RichText::new(s).monospace().size(sz);
            ui.label(t("user@nabi:~$ ").color(fg));
            ui.label(t("grep ").color(fg));
            ui.label(t("ERROR").color(egui::Color32::BLACK).background_color(mat));
            ui.label(t(" app.log").color(fg).background_color(sel));
            ui.label(t("\u{2588}").color(cur));
        });
    });
}
