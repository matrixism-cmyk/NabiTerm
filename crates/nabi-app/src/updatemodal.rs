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
        // 첫 실행 환영 화면이 떠 있는 동안은 미룬다 — 모달 둘이 겹쳐서 뜨면
        // 처음 켠 사람이 무엇을 먼저 눌러야 할지 알 수 없다(온보딩이 끝나면 그때 뜬다).
        if self.onboarding_open {
            return;
        }
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
/// 메뉴 띠 오른쪽 끝의 '업데이트' 버튼 — 새 버전이 확인된 상태에서만 그린다. 눌리면 true.
///
/// 시작 시 뜬 알림을 "다음에"로 넘기면 그 세션에선 다시 볼 길이 없었다. 여기 두면 언제든
/// 다시 열 수 있다(사용자 요청 2026-08-21).
pub(crate) fn update_button(ui: &mut egui::Ui, lang: Lang, ready: bool) -> bool {
    if !ready {
        return false;
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let b = egui::Button::new(
            egui::RichText::new(format!("\u{2b06} {}", tr(lang, "update.btn"))).color(egui::Color32::WHITE),
        )
        .fill(crate::theme_ui::OK);
        ui.add(b).on_hover_text(tr(lang, "update.btn.hint")).clicked()
    })
    .inner
}

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
            // 업데이트는 프로그램을 닫고 인스톨러를 실행한다 — 누르기 전에 반드시 알린다
            // (열려 있던 탭은 워크스페이스 복원이 켜져 있을 때만 되살아난다).
            ui.colored_label(
                crate::theme_ui::BROADCAST,
                format!("\u{26a0} {}", tr(lang, "update.restartwarn")),
            );
            ui.add_space(6.0);
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
            crate::updateui::update_bar(ui, lang, p);
        }
        UpdateStatus::Downloaded(..) => {
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
