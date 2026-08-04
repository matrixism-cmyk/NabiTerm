//! 새 버전 알림 모달 — 시작 시 새 버전이 확인되면 한 번 떠서 선택을 받는다:
//! 업데이트 / 다음에 / 일주일 후에 / 앞으로 확인 안 함.

use crate::app::NabiApp;
use nabi_i18n::{tr, Lang};
use nabi_release::{UpdateChecker, UpdateStatus};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const GREEN: egui::Color32 = egui::Color32::from_rgb(64, 190, 110);
const BLUE: egui::Color32 = egui::Color32::from_rgb(60, 150, 230);
const RED: egui::Color32 = egui::Color32::from_rgb(220, 90, 90);
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const WEEK_SECS: i64 = 7 * 86_400;

/// 현재 unix 시각(초). 스누즈 비교/설정용.
pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 모달에서 사용자가 고른 동작.
enum PromptAction {
    None,
    Updating, // 업데이트 시작(다운로드 진행을 모달에 계속 표시).
    Later,    // 다음에 — 이번 세션만 닫음(다음 실행 때 다시 알림).
    RemindWeek,
    StopCheck,
}

impl NabiApp {
    /// 새 버전 알림 모달(시작 시 1회 자동 오픈). update 루프에서 매 프레임 호출.
    pub(crate) fn show_update_prompt(&mut self, ctx: &egui::Context) {
        let status = self.updater.get_status();
        if !self.update_seen && matches!(status, UpdateStatus::Available(_)) {
            self.update_modal = true;
            self.update_seen = true; // 세션당 1회만 자동 오픈.
        }
        if !self.update_modal {
            return;
        }
        let lang = self.lang;
        let updater = self.updater.clone();
        let quit = self.update_quit.clone();
        let mut action = PromptAction::None;
        let mut open = true;
        egui::Window::new(tr(lang, "update.newtitle"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| prompt_body(ui, lang, &status, &updater, &quit, &mut action));
        if !open {
            action = PromptAction::Later; // 창 X = 다음에.
        }
        self.apply_prompt_action(action);
    }

    fn apply_prompt_action(&mut self, action: PromptAction) {
        match action {
            PromptAction::None | PromptAction::Updating => {}
            PromptAction::Later => self.update_modal = false,
            PromptAction::RemindWeek => {
                self.config.terminal.update_remind_after = now_unix() + WEEK_SECS;
                let _ = nabi_config::save(&self.config_path, &self.config);
                self.update_modal = false;
            }
            PromptAction::StopCheck => {
                self.config.terminal.auto_check_update = false;
                let _ = nabi_config::save(&self.config_path, &self.config);
                self.update_modal = false;
            }
        }
    }
}

/// 현재 업데이트 상태에 맞춰 모달 본문을 그린다(가용=선택 버튼, 다운로드=진행률).
fn prompt_body(
    ui: &mut egui::Ui,
    lang: Lang,
    status: &UpdateStatus,
    updater: &UpdateChecker,
    quit: &Arc<AtomicBool>,
    action: &mut PromptAction,
) {
    match status {
        UpdateStatus::Available(release) => {
            ui.colored_label(BLUE, format!("\u{2b06} {}", tr(lang, "update.available")));
            ui.heading(format!("v{APP_VERSION}  \u{2192}  v{}", release.version));
            if !release.notes.trim().is_empty() {
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    ui.label(release.notes.trim());
                });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let go = egui::Button::new(
                    egui::RichText::new(tr(lang, "update.now")).color(egui::Color32::WHITE),
                )
                .fill(GREEN);
                if ui.add(go).clicked() {
                    updater.download_async(release.clone(), quit.clone());
                    *action = PromptAction::Updating;
                }
                if ui.button(tr(lang, "update.later")).clicked() {
                    *action = PromptAction::Later;
                }
                if ui.button(tr(lang, "update.remindweek")).clicked() {
                    *action = PromptAction::RemindWeek;
                }
                if ui.button(tr(lang, "update.stopcheck")).clicked() {
                    *action = PromptAction::StopCheck;
                }
            });
        }
        UpdateStatus::Downloading(p) => {
            ui.label(tr(lang, "update.downloading"));
            ui.add_space(4.0);
            ui.add(
                egui::ProgressBar::new(p.percent() / 100.0)
                    .text(format!("{:.0}%  {}", p.percent(), p.display())),
            );
        }
        UpdateStatus::Downloaded(_) => {
            ui.colored_label(GREEN, format!("\u{2713} {}", tr(lang, "update.done")));
        }
        UpdateStatus::Error(msg) => {
            ui.colored_label(RED, format!("\u{2715} {msg}"));
            if ui.button(tr(lang, "update.later")).clicked() {
                *action = PromptAction::Later;
            }
        }
        _ => *action = PromptAction::Later, // Idle/Checking/UpToDate면 닫는다.
    }
}
