//! Quick Connect (SSH) 다이얼로그(다국어). 연결/저장 로직은 connectsave.rs.
//!
//! egui 중첩 클로저 self 빌림 충돌을 피하려고 `self.quick_connect`를 통째로 빌려
//! 클로저에 넘기고, 연결 동작은 클로저 밖에서 한다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// Quick Connect 폼 상태.
pub struct QuickConnect {
    pub open: bool,
    /// 저장 세션 표시 이름(비우면 user@host로 자동).
    pub name: String,
    /// 저장 세션 폴더/그룹(비우면 그룹 없음).
    pub folder: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    /// 개인키 파일 경로(비우면 비밀번호 인증).
    pub key_path: String,
    /// 점프 호스트(ProxyJump, 예: user@bastion:22). 비우면 직접 연결.
    pub jump: String,
    /// 접속 직후 자동 실행할 명령(비우면 없음).
    pub on_connect: String,
    /// 저장 시 비밀번호를 볼트에 함께 저장할지.
    pub save_password: bool,
    /// SSH 연결 시 SFTP 패널도 함께 열지.
    pub with_sftp: bool,
    /// 저장 시 FTP 세션으로 표시(연결 시 FTP 브라우저로 엶).
    pub is_ftp: bool,
    /// ssh-agent 포워딩 — 원격에서도 내 키로 서명하게 한다(믿는 서버에만).
    pub agent_forward: bool,
}

impl Default for QuickConnect {
    fn default() -> Self {
        Self {
            open: false,
            name: String::new(),
            folder: String::new(),
            host: String::new(),
            port: "22".into(),
            user: String::new(),
            password: String::new(),
            key_path: String::new(),
            jump: String::new(),
            on_connect: String::new(),
            save_password: true,
            with_sftp: false,
            is_ftp: false,
            agent_forward: false,
        }
    }
}

impl NabiApp {
    pub(crate) fn show_quick_connect(&mut self, ctx: &egui::Context) {
        if !self.quick_connect.open {
            return;
        }
        let lang = self.lang;
        // 창이 열릴 때 한 번만 에이전트에 묻는다(매 프레임 파이프를 열면 UI가 끊긴다).
        let agent_keys = self
            .agent_keys
            .get_or_insert_with(|| crate::agentkeys::probe(ctx))
            .clone();
        let mut open = true;
        let mut do_connect = false;
        let mut save = false;
        let mut sftp = false;
        let mut ftp = false;
        let mut close = false;
        let recent = self.config.terminal.recent_hosts.clone();
        let mut picked: Option<String> = None;
        let mut test_port = false;
        {
            let qc = &mut self.quick_connect;
            egui::Window::new(tr(lang, "qc.title"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if !recent.is_empty() {
                        egui::ComboBox::from_id_salt("qc_recent")
                            .selected_text(tr(lang, "qc.recent"))
                            .show_ui(ui, |ui| {
                                for r in &recent {
                                    if ui.selectable_label(false, r).clicked() {
                                        picked = Some(r.clone());
                                    }
                                }
                            });
                    }
                    egui::Grid::new("qc_grid").num_columns(2).show(ui, |ui| {
                        ui.label(tr(lang, "qc.name"));
                        ui.text_edit_singleline(&mut qc.name);
                        ui.end_row();
                        ui.label(tr(lang, "qc.folder"));
                        ui.text_edit_singleline(&mut qc.folder);
                        ui.end_row();
                        ui.label(tr(lang, "qc.host"));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut qc.host);
                            // 연결 전 host:port TCP 도달성 확인(백그라운드).
                            if ui.small_button("\u{1f50c}").on_hover_text(tr(lang, "sessions.testconn")).clicked() { test_port = true; }
                        });
                        ui.end_row();
                        ui.label(tr(lang, "qc.port"));
                        ui.text_edit_singleline(&mut qc.port);
                        ui.end_row();
                        ui.label(tr(lang, "qc.user"));
                        ui.text_edit_singleline(&mut qc.user);
                        ui.end_row();
                        ui.label(tr(lang, "qc.password"));
                        ui.add(egui::TextEdit::singleline(&mut qc.password).password(true));
                        ui.end_row();
                        ui.label(tr(lang, "qc.keyfile"));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut qc.key_path).hint_text("~/.ssh/id_ed25519"));
                            // 파일명 기반 키 종류 힌트(B5): 하드웨어키🔐 강조 / 레거시rsa·dsa 경고.
                            if let Some(h) = crate::sshkey::key_hint(&qc.key_path) {
                                let (icon, color) = if h.hardware { ("\u{1f510}", crate::theme_ui::OK) }
                                    else if h.legacy { ("\u{26a0}", crate::theme_ui::BROADCAST) } else { ("\u{1f511}", crate::theme_ui::ACCENT) };
                                ui.colored_label(color, format!("{icon} {}", h.label)).on_hover_text(tr(lang, "qc.keyhint"));
                            }
                        });
                        ui.end_row();
                        ui.label(tr(lang, "qc.jump"));
                        ui.add(egui::TextEdit::singleline(&mut qc.jump).hint_text("user@bastion:22, user@bastion2"));
                        ui.end_row();
                        ui.label(tr(lang, "qc.oncommand"));
                        ui.add(
                            egui::TextEdit::singleline(&mut qc.on_connect).hint_text("e.g. cd /var/log"),
                        );
                        ui.end_row();
                    });
                    // 둘 다 비어 있으면 ssh-agent로 붙는다(ssh(1)과 같은 규칙) — 눈에 보이게 알린다.
                    if qc.password.is_empty() && qc.key_path.trim().is_empty() {
                        ui.weak(tr(lang, "qc.agenthint"));
                        // 에이전트에 키가 없으면 이대로 붙어 봐야 실패한다 — 미리 알린다.
                        if let Some((text, warn)) = crate::agentkeys::summary(&agent_keys) {
                            let r = match warn {
                                true => ui.colored_label(egui::Color32::from_rgb(0xd0, 0x8a, 0x3a), text),
                                false => ui.weak(text),
                            };
                            let d = crate::agentkeys::detail(&agent_keys);
                            if !d.is_empty() {
                                r.on_hover_text(d);
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut qc.save_password, tr(lang, "qc.savepw"));
                        ui.checkbox(&mut qc.with_sftp, tr(lang, "qc.withsftp"));
                        ui.checkbox(&mut qc.is_ftp, tr(lang, "qc.ftpsession"));
                    });
                    // 에이전트 포워딩 — 위험을 아는 사람만 켜야 하므로 뜻을 옆에 적어 둔다.
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut qc.agent_forward, tr(lang, "qc.agentfwd"))
                            .on_hover_text(tr(lang, "qc.agentfwd.hint"));
                        // 포워딩할 에이전트가 없으면 켜도 소용없다 — 미리 알려 준다.
                        if qc.agent_forward && !nabi_ssh::agentfwd::available() {
                            ui.colored_label(crate::theme_ui::BROADCAST, tr(lang, "qc.agentfwd.noagent"));
                        }
                    });
                    // Enter로 즉시 연결, Esc로 닫기.
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        do_connect = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close = true;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "qc.connect")).clicked() {
                            do_connect = true;
                        }
                        if ui.button(tr(lang, "qc.save")).clicked() {
                            save = true;
                        }
                        if ui.button(tr(lang, "qc.saveconnect")).clicked() {
                            save = true;
                            do_connect = true;
                        }
                        if ui.button("SFTP").clicked() {
                            sftp = true;
                        }
                        if ui.button("FTP").clicked() {
                            ftp = true;
                        }
                        if ui.button(tr(lang, "qc.cancel")).clicked() {
                            close = true;
                        }
                    });
                });
        }
        // 호스트 필드에 user@host[:port]를 붙여넣으면 자동 분리(편의). '@' 있을 때만(IPv6 등 모호성 회피).
        if (do_connect || sftp || ftp || test_port) && self.quick_connect.host.contains('@') {
            if let Some(t) = crate::qcparse::parse_target(self.quick_connect.host.trim()) {
                self.quick_connect.host = t.host;
                if let Some(u) = t.user { self.quick_connect.user = u; }
                if let Some(p) = t.port { self.quick_connect.port = p.to_string(); }
            }
        }
        if test_port {
            let (h, p) = (self.quick_connect.host.clone(), self.quick_connect.port.trim().parse().unwrap_or(22));
            if !h.is_empty() { self.test_connection(h, p, ctx); }
        }
        if let Some(p) = picked {
            let (u, h, port) = crate::recent::parse_recent(&p);
            self.quick_connect.user = u;
            self.quick_connect.host = h;
            self.quick_connect.port = port;
        }
        if do_connect {
            self.do_quick_connect();
            if self.quick_connect.with_sftp {
                self.open_remote_from_qc(false);
            }
            open = false;
        }
        if save {
            self.save_current_session();
        }
        if sftp {
            self.open_remote_from_qc(false);
            open = false;
        }
        if ftp {
            // 포트가 SSH 기본값 그대로면 FTP 표준 포트(21)로 보정.
            if self.quick_connect.port.trim() == "22" {
                self.quick_connect.port = "21".into();
            }
            self.open_remote_from_qc(true);
            open = false;
        }
        if close {
            open = false;
        }
        if do_connect || sftp || ftp {
            let h = self.quick_connect.host.trim().to_string();
            if !h.is_empty() {
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
        }
        self.quick_connect.open = open;
    }
}
