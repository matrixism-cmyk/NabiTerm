//! 로컬 포트 포워딩(-L) 다이얼로그. SSH 서버로 접속 후 임시 로컬 포트를
//! remote_host:remote_port로 터널링한다(백엔드: nabi-ssh-ext start_local_forward).

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_proto::{Command, SshParams};
use std::time::Instant;

/// 로컬 포워딩 입력 폼.
pub struct ForwardForm {
    pub open: bool,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    pub remote_host: String,
    pub remote_port: String,
    /// 시작된 터널들 (id, 설명) — 중지 가능.
    pub active: Vec<(u64, String)>,
    /// 터널 id 카운터.
    pub seq: u64,
}

impl Default for ForwardForm {
    fn default() -> Self {
        Self {
            open: false,
            host: String::new(),
            port: "22".into(),
            user: String::new(),
            password: String::new(),
            remote_host: String::new(),
            remote_port: String::new(),
            active: Vec::new(),
            seq: 0,
        }
    }
}

impl NabiApp {
    /// 포워딩 다이얼로그를 연다(비어 있으면 최근 SSH 접속값으로 프리필).
    pub(crate) fn open_forward(&mut self) {
        if self.forward.host.is_empty() {
            self.forward.host = self.config.terminal.last_host.clone();
            self.forward.user = self.config.terminal.last_user.clone();
            self.forward.port = self.config.terminal.last_port.clone();
        }
        self.forward.open = true;
    }

    pub(crate) fn show_forward(&mut self, ctx: &egui::Context) {
        if !self.forward.open {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        let mut start = false;
        let mut socks = false;
        let mut remote_fwd = false;
        let mut x11 = false;
        let mut stop_id: Option<u64> = None;
        let mut close = false;
        {
            let f = &mut self.forward;
            egui::Window::new(tr(lang, "fwd.title"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("fwd_grid").num_columns(2).show(ui, |ui| {
                        ui.label(tr(lang, "qc.host"));
                        ui.text_edit_singleline(&mut f.host);
                        ui.end_row();
                        ui.label(tr(lang, "qc.port"));
                        ui.text_edit_singleline(&mut f.port);
                        ui.end_row();
                        ui.label(tr(lang, "qc.user"));
                        ui.text_edit_singleline(&mut f.user);
                        ui.end_row();
                        ui.label(tr(lang, "qc.password"));
                        ui.add(egui::TextEdit::singleline(&mut f.password).password(true));
                        ui.end_row();
                        ui.label(tr(lang, "fwd.remotehost"));
                        ui.text_edit_singleline(&mut f.remote_host);
                        ui.end_row();
                        ui.label(tr(lang, "fwd.remoteport"));
                        ui.text_edit_singleline(&mut f.remote_port);
                        ui.end_row();
                    });
                    ui.label(tr(lang, "fwd.hint"));
                    if !f.active.is_empty() {
                        ui.separator();
                        ui.label(tr(lang, "fwd.active"));
                        for (fid, a) in &f.active {
                            ui.horizontal(|ui| {
                                if ui.small_button("\u{23f9}").clicked() {
                                    stop_id = Some(*fid);
                                }
                                if ui.small_button("\u{1f4cb}").on_hover_text(tr(lang, "fwd.copyaddr")).clicked() {
                                    ui.ctx().copy_text(a.split(' ').next().unwrap_or(a).to_string()); // 로컬 주소(127.0.0.1:포트) 복사.
                                }
                                ui.label(format!("\u{1f513} {a}"));
                            });
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tr(lang, "fwd.start")).clicked() {
                            start = true;
                        }
                        if ui.button(tr(lang, "fwd.socks")).clicked() {
                            socks = true;
                        }
                        if ui.button(tr(lang, "fwd.remote")).clicked() {
                            remote_fwd = true;
                        }
                        if ui.button(tr(lang, "fwd.x11")).on_hover_text(tr(lang, "fwd.x11hint")).clicked() {
                            x11 = true;
                        }
                        if ui.button(tr(lang, "qc.cancel")).clicked() {
                            close = true;
                        }
                    });
                });
        }
        if start {
            self.start_local_forward();
            open = false;
        }
        if socks {
            self.start_dynamic_forward();
            open = false;
        }
        if x11 {
            self.start_x11_forward();
            open = false;
        }
        if remote_fwd {
            self.start_remote_forward();
            open = false;
        }
        if let Some(id) = stop_id {
            self.orch.send(Command::StopForward { id });
            self.forward.active.retain(|(i, _)| *i != id);
        }
        if close {
            open = false;
        }
        self.forward.open = open;
    }

    /// 다음 터널 id를 발급한다.
    pub(crate) fn next_fwd_id(&mut self) -> u64 {
        self.forward.seq += 1;
        self.forward.seq
    }

    /// SSH 파라미터로 X11(-X) 포워딩을 시작한다(서버 x11 채널→로컬 X 서버 127.0.0.1:6000). 원격 host/port 무시.
    fn start_x11_forward(&mut self) {
        let host = self.forward.host.trim().to_string();
        if host.is_empty() {
            return;
        }
        let port = self.forward.port.trim().parse().unwrap_or(22);
        let user = self.forward.user.trim().to_string();
        let params = SshParams::password(host, port, user, self.forward.password.clone());
        let id = self.next_fwd_id();
        self.orch.send(Command::StartX11Forward { id, params });
        self.notify = Some((tr(self.lang, "fwd.starting").to_string(), Instant::now()));
    }

    /// SSH 파라미터만으로 동적(-D) SOCKS5 프록시를 시작한다(원격 host/port는 무시).
    fn start_dynamic_forward(&mut self) {
        let host = self.forward.host.trim().to_string();
        if host.is_empty() {
            return;
        }
        let port = self.forward.port.trim().parse().unwrap_or(22);
        let user = self.forward.user.trim().to_string();
        let params = SshParams::password(host, port, user, self.forward.password.clone());
        let id = self.next_fwd_id();
        self.orch.send(Command::StartDynamicForward { id, params });
        self.notify = Some((tr(self.lang, "fwd.starting").to_string(), Instant::now()));
    }

    /// 원격(-R) 포워딩: 서버 remote_port → 로컬 127.0.0.1:remote_port.
    fn start_remote_forward(&mut self) {
        let host = self.forward.host.trim().to_string();
        let remote_port = self.forward.remote_port.trim().parse().unwrap_or(0);
        if host.is_empty() || remote_port == 0 {
            return;
        }
        let port = self.forward.port.trim().parse().unwrap_or(22);
        let user = self.forward.user.trim().to_string();
        let params = SshParams::password(host, port, user, self.forward.password.clone());
        let id = self.next_fwd_id();
        self.orch.send(Command::StartRemoteForward { id, params, remote_port });
        self.notify = Some((tr(self.lang, "fwd.starting").to_string(), Instant::now()));
    }

    fn start_local_forward(&mut self) {
        let host = self.forward.host.trim().to_string();
        let remote_host = self.forward.remote_host.trim().to_string();
        let remote_port = self.forward.remote_port.trim().parse().unwrap_or(0);
        if host.is_empty() || remote_host.is_empty() || remote_port == 0 {
            return;
        }
        let port = self.forward.port.trim().parse().unwrap_or(22);
        let user = self.forward.user.trim().to_string();
        let params = SshParams::password(host, port, user, self.forward.password.clone());
        let id = self.next_fwd_id();
        self.orch.send(Command::StartLocalForward {
            id,
            params,
            remote_host,
            remote_port,
        });
        self.notify = Some((tr(self.lang, "fwd.starting").to_string(), Instant::now()));
    }
}
