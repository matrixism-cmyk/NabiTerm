//! 설정>글꼴: 인기 코딩 폰트 클릭 다운로드 행(settingsui에서 분리 — 라인 한도).

use crate::fontinstall::{catalog, FontInstaller, InstState};
use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

const GREEN: egui::Color32 = egui::Color32::from_rgb(0x4c, 0xaf, 0x50);

/// 정밀 조절 행의 사양 — 슬라이더(굵은 스냅) + 미세 버튼 + 직접 입력 + 기본값 복원.
pub(crate) struct FineSpec {
    /// 슬라이더가 다루는 흔한 범위(드래그 해상도를 여기에 씀).
    pub coarse: std::ops::RangeInclusive<f32>,
    /// 직접 입력까지 허용하는 전체 범위(런타임 클램프와 일치시킬 것).
    pub full: std::ops::RangeInclusive<f32>,
    /// 슬라이더 스냅 간격(깔끔한 값에 멈춰 드래그가 편하다).
    pub snap: f64,
    /// −/+ 버튼 한 번의 증감.
    pub fine: f32,
    pub decimals: usize,
    pub suffix: &'static str,
    pub default: f32,
}

/// 슬라이더로 굵게, −/+로 미세하게, 숫자 칸으로 정확하게 — 세 손잡이를 한 행에 담는다.
/// 스냅 슬라이더만으로는 그 사이 값을 만들 수 없다는 보고(정밀 제어 요청)로 도입.
pub(crate) fn fine_row(ui: &mut egui::Ui, v: &mut f32, s: &FineSpec, reset_hint: &str) {
    ui.horizontal(|ui| {
        let (lo, hi) = (*s.full.start(), *s.full.end());
        if ui.small_button("\u{2212}").on_hover_text(format!("-{}", s.fine)).clicked() { *v = (*v - s.fine).max(lo); }
        ui.spacing_mut().slider_width = 190.0;
        ui.add(egui::Slider::new(v, s.coarse.clone()).step_by(s.snap).fixed_decimals(s.decimals).suffix(s.suffix).show_value(false));
        if ui.small_button("+").on_hover_text(format!("+{}", s.fine)).clicked() { *v = (*v + s.fine).min(hi); }
        // 직접 입력: 스냅 없이 아무 값이나(전체 범위 안에서). 드래그도 미세 단위로 움직인다.
        ui.add(egui::DragValue::new(v).range(s.full.clone()).speed(f64::from(s.fine) / 4.0).fixed_decimals(s.decimals).suffix(s.suffix));
        if ui.small_button("\u{21ba}").on_hover_text(reset_hint).clicked() { *v = s.default; }
    });
    ui.end_row();
}

/// UI 배율 행 — 슬라이더 0.05 스냅, 버튼 0.01, 입력은 0.5~3.0(런타임 클램프와 동일).
pub(crate) fn ui_scale_row(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    ui.label(tr(lang, "settings.uiscale"));
    let spec = FineSpec {
        coarse: 0.8..=2.0, full: 0.5..=3.0, snap: 0.05, fine: 0.01,
        decimals: 2, suffix: "\u{00d7}", default: 1.0,
    };
    fine_row(ui, &mut cfg.appearance.ui_scale, &spec, tr(lang, "settings.resetdefault"));
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
