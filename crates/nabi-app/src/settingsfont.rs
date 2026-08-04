//! 설정>글꼴: 인기 코딩 폰트 클릭 다운로드 행(settingsui에서 분리 — 라인 한도).

use crate::fontinstall::{catalog, FontInstaller, InstState};
use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

const GREEN: egui::Color32 = egui::Color32::from_rgb(0x4c, 0xaf, 0x50);

/// UI 배율 조절 행 — 슬라이더는 0.1 단위로 스냅(드래그가 깔끔한 중간값에 멈춰 조절 쉬움),
/// −/+ 버튼은 0.02씩 더 미세한 조정(둘 조합).
pub(crate) fn ui_scale_row(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.uiscale"));
    ui.horizontal(|ui| {
        if ui.small_button("\u{2212}").on_hover_text("-0.02").clicked() { cfg.appearance.ui_scale = (cfg.appearance.ui_scale - 0.02).max(0.8); }
        ui.spacing_mut().slider_width = 230.0;
        ui.add(egui::Slider::new(&mut cfg.appearance.ui_scale, 0.8..=2.0).step_by(0.1).fixed_decimals(2).suffix("\u{00d7}"));
        if ui.small_button("+").on_hover_text("+0.02").clicked() { cfg.appearance.ui_scale = (cfg.appearance.ui_scale + 0.02).min(2.0); }
    });
    ui.end_row();
}

/// 카탈로그의 각 폰트를 설치 상태에 따라 버튼으로 그린다(다운로드/사용/받는 중/재시도).
/// 다운로드 완료 시 cfg.font_family를 그 경로로 바꿔 즉시 적용되게 한다.
pub(crate) fn font_get_row(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang, inst: &FontInstaller) {
    let fonts = crate::fonts::list_monospace_fonts();
    let ctx = ui.ctx().clone();
    ui.vertical(|ui| {
        for def in catalog() {
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 18.0], egui::Label::new(def.label).truncate());
                // 이미 설치됨(스캔 목록에 패밀리명 존재) → 클릭으로 선택.
                let installed = fonts.iter().find(|(n, _)| n.eq_ignore_ascii_case(def.label)).map(|(_, p)| p);
                match inst.status(def.label) {
                    Some(InstState::Working) => {
                        ui.add_enabled(false, egui::Button::new(tr(lang, "font.getting")));
                    }
                    Some(InstState::Done(path)) => {
                        cfg.appearance.font_family = path; // 받은 폰트를 즉시 본문 글꼴로.
                        crate::fonts::invalidate_font_cache(); // 목록 재스캔(새 폰트 반영).
                        inst.clear(def.label);
                        ui.colored_label(GREEN, format!("\u{2713} {}", tr(lang, "font.installed")));
                    }
                    Some(InstState::Error(e)) => {
                        if ui.button(tr(lang, "font.retry")).on_hover_text(e).clicked() {
                            inst.install_async(def.label, &ctx);
                        }
                    }
                    None if installed.is_some() => {
                        if ui.button(tr(lang, "font.use")).clicked() {
                            cfg.appearance.font_family = installed.cloned().unwrap_or_default();
                        }
                        ui.colored_label(GREEN, format!("\u{2713} {}", tr(lang, "font.installed")));
                    }
                    None => {
                        if ui.button(format!("\u{2b07} {}", tr(lang, "font.download"))).clicked() {
                            inst.install_async(def.label, &ctx);
                        }
                    }
                }
            });
        }
    });
}
