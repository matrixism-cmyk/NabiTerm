//! 세션별 환경변수 편집 창 — 세션 우클릭에서 연다.
//!
//! 자동 터널 편집(`autofwdui`)과 같은 자리·같은 모양이다. 둘 다 "세션에 딸리지만 세션
//! 파일에는 없는 것"이라 설정에 세션 이름을 열쇠로 산다(`session_env`).
//!
//! 여러 줄 상자 하나로 받는다. 줄마다 `KEY=VALUE`이고, 잘못 적은 줄은 **보내지 않되
//! 지우지도 않는다** — 사용자가 적은 것을 프로그램이 말없이 고치면 신뢰를 잃는다.
//! 대신 아래에 몇 줄이 나가고 무엇이 걸러지는지 미리 보여 준다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 이 세션의 환경변수를 편집한다.
    pub(crate) fn open_session_env(&mut self, session: String) {
        self.env_edit = Some(session);
    }

    /// 열려 있으면 그린다.
    pub(crate) fn show_session_env(&mut self, ctx: &egui::Context) {
        let Some(name) = self.env_edit.clone() else { return };
        let lang = self.lang;
        let mut open = true;
        let mut text = self.config.terminal.session_env.get(&name).cloned().unwrap_or_default();
        let mut changed = false;
        egui::Window::new(format!("{} — {name}", tr(lang, "sessenv.title")))
            .open(&mut open)
            .collapsible(false)
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.label(tr(lang, "sessenv.hint"));
                ui.add_space(4.0);
                changed |= ui
                    .add(
                        egui::TextEdit::multiline(&mut text)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .code_editor()
                            .hint_text("LANG=ko_KR.UTF-8\nDEPLOY_USER=kim"),
                    )
                    .changed();
                ui.add_space(6.0);

                // 무엇이 실제로 나가는지 미리 보여 준다 — 서버가 조용히 무시하면 원인을
                // 찾을 길이 없으므로, 최소한 우리가 보낸 것은 알 수 있어야 한다.
                let pairs = nabi_ssh::envvars::parse(&text);
                let lines = text.lines().filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#')
                });
                let dropped = lines.count().saturating_sub(pairs.len());
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}: {}", tr(lang, "sessenv.willsend"), pairs.len()));
                    if dropped > 0 {
                        ui.colored_label(
                            crate::theme_ui::BROADCAST,
                            format!("{} {dropped}", tr(lang, "sessenv.dropped")),
                        );
                    }
                });
                for (k, _) in pairs.iter().take(8) {
                    ui.weak(format!("  {k}"));
                }
                ui.add_space(6.0);
                ui.weak(tr(lang, "sessenv.accepthint"));
            });

        if changed {
            match text.trim().is_empty() {
                true => {
                    self.config.terminal.session_env.remove(&name);
                }
                false => {
                    self.config.terminal.session_env.insert(name.clone(), text);
                }
            }
            self.save_config();
        }
        if !open {
            self.env_edit = None;
        }
    }
}
