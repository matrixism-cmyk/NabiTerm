//! 자동 업데이트 UI 섹션(설정 > 동작) — nabidrive식 컬러 상태 표시.
//! 확인/다운로드는 nabi_release::UpdateChecker(백그라운드 스레드)에 위임.

use nabi_i18n::{tr, Lang};
use nabi_release::{UpdateChecker, UpdateStatus};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const TEAL: egui::Color32 = egui::Color32::from_rgb(0, 150, 190);
const GREEN: egui::Color32 = egui::Color32::from_rgb(64, 190, 110);
const BLUE: egui::Color32 = egui::Color32::from_rgb(60, 150, 230);
const RED: egui::Color32 = egui::Color32::from_rgb(220, 90, 90);

/// 현재 버전(설치된 빌드).
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 업데이트 섹션을 그린다. updater/quit는 앱에서 클론해 넘긴다(Arc 공유).
pub(crate) fn update_section(
    ui: &mut egui::Ui,
    lang: Lang,
    updater: &UpdateChecker,
    quit: &Arc<AtomicBool>,
) {
    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr(lang, "update.section")).strong().color(TEAL));
            ui.weak(format!("v{APP_VERSION}"));
        });
        ui.add_space(4.0);
        match updater.get_status() {
            UpdateStatus::Idle => {
                draw_check_button(ui, lang, updater);
            }
            UpdateStatus::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr(lang, "update.checking"));
                });
            }
            UpdateStatus::UpToDate => {
                ui.colored_label(GREEN, format!("\u{2713} {}", tr(lang, "update.uptodate")));
                draw_check_button(ui, lang, updater);
            }
            UpdateStatus::Available(release) => {
                ui.colored_label(
                    BLUE,
                    format!("\u{2b06} {} v{}", tr(lang, "update.available"), release.version),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let go = egui::Button::new(
                        egui::RichText::new(tr(lang, "update.now")).color(egui::Color32::WHITE),
                    )
                    .fill(GREEN);
                    if ui.add(go).clicked() {
                        updater.download_async(release.clone(), quit.clone());
                    }
                    if ui.button(tr(lang, "update.page")).clicked() {
                        open_url(nabi_release::RELEASES_URL);
                    }
                });
            }
            UpdateStatus::Downloading(p) => {
                ui.label(tr(lang, "update.downloading"));
                ui.add_space(4.0);
                let bar = egui::ProgressBar::new(p.percent() / 100.0)
                    .text(format!("{:.0}%  {}", p.percent(), p.display()));
                ui.add(bar);
            }
            UpdateStatus::Downloaded(path, want) => {
                ui.colored_label(GREEN, format!("\u{2713} {}", tr(lang, "update.done")));
                if ui.button(tr(lang, "update.install")).clicked() {
                    // 실행 전 SHA-256 대조(불일치·미공지면 실행하지 않는다).
                    let _ = nabi_release::launch_installer(&path, want.as_deref(), quit);
                }
            }
            UpdateStatus::Error(msg) => {
                ui.colored_label(RED, format!("\u{2715} {msg}"));
                // 자동 업데이트가 막히는 환경(HTTPS 검사 프록시·백신)이 실제로 있다.
                // 다시 확인만 제공하면 막다른 길이 되므로 직접 받을 길을 함께 연다.
                ui.horizontal(|ui| {
                    draw_check_button(ui, lang, updater);
                    if ui.button(tr(lang, "update.page")).clicked() {
                        open_url(nabi_release::RELEASES_URL);
                    }
                });
            }
        }
    });
}

fn draw_check_button(ui: &mut egui::Ui, lang: Lang, updater: &UpdateChecker) {
    ui.add_space(4.0);
    let btn =
        egui::Button::new(egui::RichText::new(tr(lang, "update.check")).color(egui::Color32::WHITE))
            .fill(TEAL);
    if ui.add(btn).clicked() {
        updater.check_async();
    }
}

fn open_url(url: &str) {
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
}
