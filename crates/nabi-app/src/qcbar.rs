//! FileZilla식 퀵커넥트 바 — 메뉴 아래 인라인 호스트/사용자/비밀번호/포트 + 즉시 연결.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 퀵커넥트 바를 그린다(설정 `show_quickconnect_bar`가 켜진 경우).
    /// Enter 또는 연결 버튼 = SSH 터미널, 🖧 = SFTP 브라우저로 연결.
    pub(crate) fn show_quickconnect_bar(&mut self, ctx: &egui::Context) {
        if !self.config.appearance.show_quickconnect_bar {
            return;
        }
        let lang = self.lang;
        let (mut go, mut sftp, mut host_blur) = (false, false, false);
        let frame = egui::Frame::NONE
            .fill(crate::theme_ui::STATUS_FILL)
            .inner_margin(egui::Margin::symmetric(8, 4));
        egui::TopBottomPanel::top("qc_bar").frame(frame).show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(crate::theme_ui::TEXT_BRIGHT);
            ui.horizontal(|ui| {
                let enter = |ui: &egui::Ui, r: &egui::Response| {
                    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                };
                let qc = &mut self.quick_connect;
                let r = ui.add(
                    egui::TextEdit::singleline(&mut qc.host)
                        .hint_text(tr(lang, "qc.host"))
                        .desired_width(150.0),
                );
                go |= enter(ui, &r);
                host_blur = r.lost_focus(); // 호스트 칸을 벗어나면 연결 문자열 분해 시도.
                let r = ui.add(
                    egui::TextEdit::singleline(&mut qc.user)
                        .hint_text(tr(lang, "qc.user"))
                        .desired_width(100.0),
                );
                go |= enter(ui, &r);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut qc.password)
                        .hint_text(tr(lang, "qc.password"))
                        .password(true)
                        .desired_width(100.0),
                );
                go |= enter(ui, &r);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut qc.port)
                        .hint_text(tr(lang, "qc.port"))
                        .desired_width(46.0),
                );
                go |= enter(ui, &r);
                if ui.button(tr(lang, "qc.connect")).clicked() {
                    go = true;
                }
                if ui
                    .button("\u{1f5a7}")
                    .on_hover_text(tr(lang, "sessions.opensftp"))
                    .clicked()
                {
                    sftp = true;
                }
            });
        });
        // 호스트 칸을 확정할 때: `user@host:port`/`ssh …`는 분해하고, 단순 별칭은
        // ~/.ssh/config에서 HostName/User/Port를 해석한다.
        if go || sftp || host_blur {
            let raw = self.quick_connect.host.trim().to_string();
            if crate::qcparse::looks_full(&raw) {
                if let Some(parsed) = crate::qcparse::parse_connect(&raw) {
                    let qc = &mut self.quick_connect;
                    qc.host = parsed.host;
                    if let Some(u) = parsed.user { qc.user = u; }
                    if let Some(port) = parsed.port { qc.port = port.to_string(); }
                    if let Some(j) = parsed.jump { qc.jump = j; } // `ssh -J ...` 붙여넣기의 점프 호스트.
                }
            } else if !raw.is_empty() {
                self.resolve_qc_alias(&raw);
            }
        }
        if !(go || sftp) {
            return;
        }
        if self.quick_connect.host.trim().is_empty() {
            return; // 호스트 없이 연결 시도 방지.
        }
        if go {
            self.do_quick_connect();
            if self.quick_connect.with_sftp {
                self.open_remote_from_qc(false);
            }
        }
        if sftp {
            self.open_remote_from_qc(false);
        }
        // 최근 호스트 기록(빠른 연결 다이얼로그와 동일 규칙).
        let h = self.quick_connect.host.trim().to_string();
        let u = self.quick_connect.user.trim();
        let p = self.quick_connect.port.trim();
        let entry = if u.is_empty() {
            format!("{h}:{p}")
        } else {
            format!("{u}@{h}:{p}")
        };
        crate::recent::push_recent(&mut self.config.terminal.recent_hosts, entry, 12);
        let _ = nabi_config::save(&self.config_path, &self.config);
    }

    /// 호스트 칸의 단순 별칭을 ~/.ssh/config에서 해석해 host/user/port를 채운다.
    /// 일치하는 Host가 없으면(또는 config 없음) 입력을 그대로 둔다(일반 호스트명으로 취급).
    fn resolve_qc_alias(&mut self, alias: &str) {
        let path = crate::browser::home_dir().join(".ssh").join("config");
        let Ok(content) = std::fs::read_to_string(&path) else { return };
        if let Some((host, user, port, _key)) = crate::sshconfig::resolve_alias(&content, alias) {
            let qc = &mut self.quick_connect;
            qc.host = host;
            if qc.user.trim().is_empty() && !user.is_empty() {
                qc.user = user; // 직접 입력한 사용자가 우선.
            }
            qc.port = port.to_string();
        }
    }
}
