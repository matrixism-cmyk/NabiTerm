//! 도움말 대화상자 — 좌측 내비(정보/단축키/AI 제어). 본문은 [`crate::helppages`].

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    pub(crate) fn show_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            self.help_update_checked = false; // 닫혀 있으면 리셋 → 다음에 열 때 재검사.
            return;
        }
        let lang = self.lang;
        let cfg_dir = self.config_path.parent().map(|p| p.to_path_buf());
        let offline = self.config.terminal.offline_mode;
        let updater = self.updater.clone();
        let quit = self.update_quit.clone();
        let cat_id = egui::Id::new("help_cat");
        let mut cat = ctx.data(|d| d.get_temp::<usize>(cat_id)).unwrap_or(0);
        // 정보 페이지를 볼 때 항상 최신 버전을 자동 검사(진입당 1회 — 다운로드 중이면 건드리지 않음).
        if cat == 0 && !self.help_update_checked {
            self.help_update_checked = true;
            if !matches!(
                updater.get_status(),
                nabi_release::UpdateStatus::Checking
                    | nabi_release::UpdateStatus::Downloading(_)
                    | nabi_release::UpdateStatus::Downloaded(..)
            ) {
                // 여기는 사용자가 도움말을 **연** 것이므로 오프라인이어도 막지 않는다.
                // 눌렀는데 아무 일도 없으면 그건 보호가 아니라 고장이다.
                updater.check_async();
            }
        }
        let (mut open, mut copy, mut save) = (true, false, false);
        // AI CLI 자동 업데이트 설정은 이 창에서 바꿀 수 있다 — 바뀌면 즉시 저장한다.
        let mut open_env = false; // 도움말에서 환경 관리자로 건너가기.
        let mut open_logs = false; // 도움말에서 '진단 로그'를 눌렀는가.
        egui::Window::new(tr(lang, "help.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size([660.0, 540.0]) // 고정 폭 — 긴 설명이 가로로 뻗지 않고 줄바꿈되게.
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(120.0);
                        for (i, key) in crate::helppages::HELP_CATS.iter().enumerate() {
                            let lbl = egui::Button::selectable(cat == i, tr(lang, key));
                            if ui.add_sized([112.0, 28.0], lbl).clicked() {
                                cat = i;
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(500.0);
                        ui.set_max_width(500.0); // 본문 폭 고정 → 라벨 자동 줄바꿈.
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(480.0)
                            .show(ui, |ui| match cat {
                                0 => crate::helppages::about_page(
                                    ui, lang, cfg_dir.as_deref(), &updater, &quit, &mut open_logs,
                                ),
                                1 => crate::helppages::shortcut_page(ui, lang),
                                2 => crate::helppages::features_page(ui, lang),
                                3 => crate::helppages::agent_page(
                                    ui, lang, &mut copy, &mut save, &mut open_env,
                                ),
                                4 => crate::helppages::network_page(ui, lang, offline),
                                _ => crate::helppages::licenses_page(ui, lang),
                            });
                    });
                });
            });
        ctx.data_mut(|d| d.insert_temp(cat_id, cat));
        if open_logs {
            self.open_log_view();
        }
        if open_env {
            self.open_env_mgr();
        }
        if copy {
            ctx.copy_text(crate::agentguide::agent_guide_md(&Self::exe_path()));
            self.notify = Some((tr(lang, "help.agent.copied").to_string(), std::time::Instant::now()));
        }
        if save {
            self.save_agent_guide();
        }
        self.about_open = open;
    }

    /// AI 사용설명 MD를 설정 폴더에 저장하고 탐색기에서 보여준다(토스트로 경로 안내).
    fn save_agent_guide(&mut self) {
        let dir = self
            .config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let path = dir.join("nabiterm-agent-guide.md");
        let msg = match std::fs::write(&path, crate::agentguide::agent_guide_md(&Self::exe_path())) {
            Ok(()) => {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{}", path.display()))
                    .spawn();
                format!("{} {}", tr(self.lang, "help.agent.saved"), path.display())
            }
            Err(e) => format!("\u{2715} {e}"),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }

    /// 실행 중인 nabiTerm exe의 절대 경로(실패 시 "nabi"). AI 사용설명에 주입한다.
    fn exe_path() -> String {
        std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "nabi".to_string())
    }
}
