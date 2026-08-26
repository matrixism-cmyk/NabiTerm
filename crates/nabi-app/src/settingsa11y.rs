//! **접근성 설정 페이지** — 색·크기·움직임에 관한 것을 한자리에.
//!
//! 여기 있는 것 중 일부는 원래 다른 페이지에 있었다(커서 깜빡임은 '모양'에). 흩어 두면
//! **필요한 사람이 못 찾는다.** 색약 사용자가 "이 프로그램은 나를 위한 게 없다"고 판단하는
//! 데는 설정 화면을 한 번 훑는 시간이면 충분하다.
//!
//! 그래서 옮기지 않고 **모아 둔다** — 원래 자리에도 그대로 있고, 여기서도 보인다. 같은
//! 값을 두 화면이 편집하지만 대상은 하나이므로 어긋날 일이 없다.
//!
//! ## 기본값은 건드리지 않는다
//!
//! 전부 "끄기"가 기본이다. 켠 사람에게만 화면이 달라진다 — 지금 쓰는 사람의 화면이
//! 바뀌면 그건 개선이 아니라 회귀다.

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};
use nabi_types::palette::Palette;

/// 접근성 페이지의 항목들.
pub(crate) fn a11y_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.palette"));
    let cur = Palette::from_name(&cfg.appearance.palette);
    egui::ComboBox::from_id_salt("a11y_palette")
        .selected_text(tr(lang, palette_key(cur)))
        .show_ui(ui, |ui| {
            for p in [Palette::Standard, Palette::Deuteranopia, Palette::HighContrast] {
                if ui.selectable_label(cur == p, tr(lang, palette_key(p))).clicked() {
                    cfg.appearance.palette = p.as_str().to_string();
                }
            }
        });
    ui.end_row();

    // 배율은 '모양' 페이지가 쓰는 그 줄을 **그대로** 부른다 — 새로 그리면 두 화면의
    // 눈금·범위·기본값이 언젠가 어긋난다.
    crate::settingsfont::ui_scale_row(ui, cfg, lang);
    ui.end_row();

    // 깜빡임은 '모양'에도 있다 — 여기서도 보이게 둔다(같은 값을 가리킨다).
    ui.label(tr(lang, "settings.cursorblink"));
    ui.checkbox(&mut cfg.appearance.cursor_blink, "")
        .on_hover_text(tr(lang, "settings.blinkhint"));
    ui.end_row();

    ui.label(tr(lang, "settings.a11ymarks"));
    ui.checkbox(&mut cfg.appearance.symbol_cues, "")
        .on_hover_text(tr(lang, "settings.a11ymarkshint"));
    ui.end_row();

    // 지금 테마가 읽을 만한지 그 자리에서 알려 준다 — 색을 고르고 나서야 아는 것보다 낫다.
    contrast_note(ui, cfg, lang);
}

/// 팔레트 이름의 i18n 키.
fn palette_key(p: Palette) -> &'static str {
    match p {
        Palette::Standard => "settings.palette.std",
        Palette::Deuteranopia => "settings.palette.deuter",
        Palette::HighContrast => "settings.palette.high",
    }
}

/// 지금 전경/배경 대비를 재서 낮으면 경고한다.
///
/// 값을 함께 보여 주는 까닭: "낮습니다"만으로는 얼마나 고쳐야 하는지 알 수 없다.
fn contrast_note(ui: &mut egui::Ui, cfg: &AppConfig, lang: Lang) {
    let fg = hex_rgb(&cfg.appearance.fg_color);
    let bg = hex_rgb(&cfg.appearance.bg_color);
    let (Some(fg), Some(bg)) = (fg, bg) else { return };
    let r = nabi_types::contrast::contrast_ratio(fg, bg);
    ui.label(tr(lang, "settings.contrast"));
    let txt = format!("{r:.1}:1");
    match r >= 4.5 {
        true => ui.colored_label(crate::theme_ui::OK, format!("\u{2713} {txt}")),
        false => ui
            .colored_label(crate::theme_ui::ERR, format!("\u{26a0} {txt}"))
            .on_hover_text(tr(lang, "settings.contrastlow")),
    };
    ui.end_row();
}

/// `#rrggbb`(또는 `rrggbb`)를 숫자 셋으로. 못 읽으면 None.
pub(crate) fn hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

#[cfg(test)]
mod tests {
    use super::hex_rgb;

    #[test]
    fn a_hex_colour_is_read_with_or_without_the_hash() {
        assert_eq!(hex_rgb("#1e1e1e"), Some((0x1e, 0x1e, 0x1e)));
        assert_eq!(hex_rgb("e5e5e5"), Some((0xe5, 0xe5, 0xe5)));
        assert_eq!(hex_rgb("  #FFFFFF  "), Some((255, 255, 255)));
    }

    /// 못 읽는 값은 **지어내지 않는다** — 대비 경고가 거짓이 되면 안 된다.
    #[test]
    fn a_bad_colour_is_refused() {
        assert_eq!(hex_rgb(""), None);
        assert_eq!(hex_rgb("#fff"), None, "세 자리 표기는 우리 설정 꼴이 아니다");
        assert_eq!(hex_rgb("#gggggg"), None);
        assert_eq!(hex_rgb("#1e1e1e1e"), None);
    }
}
