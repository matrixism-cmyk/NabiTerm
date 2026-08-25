//! 환경 관리자 창 그리기 — 목록·설치 버튼·진행률.

use crate::app::NabiApp;
use crate::envcat::{Group, TOOLS};
use crate::envstate::EnvState;
use nabi_i18n::{tr, Lang};

impl NabiApp {
    /// 도구▸환경 관리자.
    pub(crate) fn open_env_mgr(&mut self) {
        self.env_mgr = Some(crate::envmgr::EnvMgr::new());
    }

    /// 열려 있으면 그린다.
    ///
    /// `self.env_mgr`를 잠시 꺼내 놓고 그린다 — 창 안에서 설정(AI CLI 자동 업데이트)을
    /// 만져야 하는데, 창 상태를 빌린 채로는 `self`를 다시 만질 수 없기 때문이다.
    pub(crate) fn show_env_mgr(&mut self, ctx: &egui::Context) {
        let Some(mut mgr) = self.env_mgr.take() else { return };
        let lang = self.lang;
        let mut open = true;
        if mgr.poll() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        if mgr.dirty && !mgr.busy() {
            mgr.rescan();
        }
        let st = mgr.scan.lock().map(|s| s.clone()).unwrap_or_default();
        let mut auto_cli = self.config.terminal.ai_cli_auto_update;
        let mut cfg_changed = false;
        let mut pick: Option<(String, String, String)> = None; // (라벨, 스크립트, 첫 메시지)
        egui::Window::new(tr(lang, "env.title"))
            .open(&mut open)
            .default_size([720.0, 560.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(tr(lang, "env.intro"));
                ui.add_space(6.0);
                if let Some((frac, label)) = mgr.progress() {
                    ui.add(egui::ProgressBar::new(frac).text(label).desired_height(18.0));
                    ui.add_space(4.0);
                } else if let Some((ok, msg)) = &mgr.note {
                    ui.colored_label(result_color(*ok), msg);
                    ui.add_space(4.0);
                }
                if !st.done {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr(lang, "env.scanning"));
                    });
                    return;
                }
                ui.separator();
                egui::ScrollArea::vertical().id_salt("env_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    pick = body(ui, lang, &st, mgr.busy());
                    // AI CLI도 결국 "이 PC에 도구를 갖추는" 같은 일이라 여기로 모았다
                    // (전에는 도움말 안에 있었다 — 설치 동작이 읽는 곳에 있으면 다시 못 찾는다).
                    heading(ui, lang, "env.grp.ai");
                    cfg_changed |= crate::aiclipage::ai_cli_manager(ui, lang, &mut auto_cli);
                });
            });
        if let Some((label, script, first)) = pick {
            mgr.start(label, script, first);
        }
        if cfg_changed {
            self.config.terminal.ai_cli_auto_update = auto_cli;
            let _ = nabi_config::save(&self.config_path, &self.config);
        }
        if open {
            self.env_mgr = Some(mgr);
        }
    }
}

fn result_color(ok: bool) -> egui::Color32 {
    match ok {
        true => egui::Color32::from_rgb(0x3c, 0xa8, 0x55),
        false => egui::Color32::from_rgb(0xd0, 0x4a, 0x3a),
    }
}

/// 세 묶음을 차례로 그리고, 눌린 작업 하나를 돌려준다.
fn body(ui: &mut egui::Ui, lang: Lang, st: &EnvState, busy: bool) -> Option<(String, String, String)> {
    let mut pick = None;
    for (g, key) in [(Group::Pkg, "env.grp.pkg"), (Group::Shell, "env.grp.shell"), (Group::DevTool, "env.grp.dev")] {
        heading(ui, lang, key);
        for t in TOOLS.iter().filter(|t| t.group == g) {
            if let Some(p) = tool_row(ui, lang, t, st, busy) {
                pick = Some(p);
            }
        }
        ui.add_space(8.0);
    }
    heading(ui, lang, "env.grp.wsl");
    if st.distros.is_empty() {
        ui.weak(tr(lang, "env.wsl.none"));
    } else {
        ui.weak(tr(lang, "env.wsl.hint")); // 재부팅이 필요할 수 있다는 안내.
    }
    for d in &st.distros {
        let have = st.wsl_installed.iter().any(|i| i.eq_ignore_ascii_case(&d.name));
        ui.horizontal(|ui| {
            status_dot(ui, have);
            ui.label(&d.friendly);
            ui.weak(format!("({})", d.name));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if have {
                    ui.weak(tr(lang, "env.installed"));
                } else if ui.add_enabled(!busy, egui::Button::new(tr(lang, "env.install"))).clicked() {
                    let s = crate::envstate::distro_script(&d.name, st.has_wsl);
                    pick = Some((d.friendly.clone(), s, tr(lang, "env.starting").to_string()));
                }
            });
        });
    }
    pick
}

fn heading(ui: &mut egui::Ui, lang: Lang, key: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tr(lang, key)).strong());
    ui.separator();
}

/// 도구 한 줄. 눌렸으면 (라벨, 스크립트, 첫 메시지).
fn tool_row(
    ui: &mut egui::Ui,
    lang: Lang,
    t: &crate::envcat::Tool,
    st: &EnvState,
    busy: bool,
) -> Option<(String, String, String)> {
    let have = st.installed.iter().any(|i| i == t.id);
    let mut pick = None;
    ui.horizontal(|ui| {
        status_dot(ui, have);
        ui.label(t.name).on_hover_text(tr(lang, t.desc));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 윈도우에서 못 쓰는 것은 버튼 대신 이유를 보인다 — 되는 척하지 않는다.
            if let Some(why) = t.unavailable {
                ui.weak(tr(lang, why));
                return;
            }
            if have {
                if let Some(s) = crate::envrun::remove_script(t, st.has_winget) {
                    if ui.add_enabled(!busy, egui::Button::new(tr(lang, "env.remove"))).clicked() {
                        pick = Some((t.name.to_string(), s, tr(lang, "env.starting").to_string()));
                    }
                }
                ui.weak(tr(lang, "env.installed"));
                return;
            }
            match crate::envrun::install_script(t, st.has_winget) {
                Some(s) => {
                    if ui.add_enabled(!busy, egui::Button::new(tr(lang, "env.install"))).clicked() {
                        pick = Some((t.name.to_string(), s, tr(lang, "env.starting").to_string()));
                    }
                }
                // winget 통로밖에 없는데 winget이 없다 — 무엇부터 해야 하는지 말해 준다.
                None => {
                    ui.weak(tr(lang, "env.needwinget"));
                }
            }
        });
    });
    pick
}

fn status_dot(ui: &mut egui::Ui, on: bool) {
    let (mark, color) = match on {
        true => ("\u{25cf}", egui::Color32::from_rgb(0x3c, 0xa8, 0x55)),
        false => ("\u{25cb}", ui.visuals().weak_text_color()),
    };
    ui.colored_label(color, mark);
}
