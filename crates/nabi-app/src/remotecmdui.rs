//! 원격 명령의 확인창과 결과 창(명령 짓기·인용은 `remotecmd`).
//!
//! ## 실행하기 전에 전문을 보여 준다
//!
//! 서버에서 명령을 도는 일이다. 붙여넣기 확인창을 만든 것과 같은 생각으로, **무엇이
//! 실행될지 글자 그대로** 보여 주고 나서 묻는다. 인용이 어떻게 됐는지도 그 화면에서 보인다.
//!
//! 파일을 바꾸는 명령(압축·해제)은 표시를 하나 더 단다. 보기만 하는 명령(해시·용량)과
//! 같은 무게로 물으면, 매번 같은 확인창을 눌러 넘기다가 정작 위험한 것도 넘기게 된다.

use crate::app::NabiApp;
use crate::remotecmd::{build, RemoteOp, OPS};
use nabi_i18n::tr;

/// 확인 대기 중인 명령.
#[derive(Clone)]
pub(crate) struct PendingCmd {
    pub op: RemoteOp,
    pub cmd: String,
    pub files: usize,
}

/// 실행 결과.
#[derive(Clone, Default)]
pub(crate) struct CmdResult {
    pub cmd: String,
    pub out: String,
    pub code: Option<i32>,
    /// 보내 놓고 답을 기다리는 중.
    pub running: bool,
}

impl NabiApp {
    /// 고른 파일들에 대해 명령 하나를 준비한다(아직 실행하지 않는다).
    pub(crate) fn prepare_remote_cmd(&mut self, op: RemoteOp, names: Vec<String>) {
        if names.is_empty() {
            return;
        }
        let cmd = build(&op, &self.sftp.path, &names);
        self.rcmd_pending = Some(PendingCmd { op, cmd, files: names.len() });
    }

    /// 확인창 — **전문을 그대로** 보여 주고 묻는다.
    pub(crate) fn show_remote_cmd_confirm(&mut self, ctx: &egui::Context) {
        let Some(p) = self.rcmd_pending.clone() else { return };
        let lang = self.lang;
        let (mut run, mut cancel) = (false, false);
        crate::modal::foreground_modal(ctx, "rcmd_confirm", |ui| {
            ui.heading(tr(lang, "rcmd.title"));
            ui.label(format!("{}: {}", tr(lang, "rcmd.files"), p.files));
            if p.op.mutates {
                // 바꾸는 명령은 무게를 달리 한다 — 안 그러면 확인창이 습관이 된다.
                ui.add_space(4.0);
                ui.colored_label(crate::theme_ui::ERR, format!("\u{26a0} {}", tr(lang, "rcmd.mutates")));
            }
            ui.add_space(6.0);
            ui.label(tr(lang, "rcmd.willrun"));
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&p.cmd).monospace()).wrap());
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "rcmd.run")).clicked() {
                    run = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
        if cancel {
            self.rcmd_pending = None;
        }
        if run {
            self.rcmd_pending = None;
            self.send_remote_cmd(&p.cmd);
        }
    }

    /// 명령을 보낸다. 결과는 `SftpExecDone`으로 돌아온다.
    fn send_remote_cmd(&mut self, cmd: &str) {
        let Some(id) = self.sftp.id else { return };
        self.rcmd_result = Some(CmdResult { cmd: cmd.to_string(), running: true, ..Default::default() });
        self.orch.send(nabi_proto::Command::SftpExec { id, cmd: cmd.to_string() });
    }

    /// 결과 도착.
    pub(crate) fn on_remote_cmd_done(&mut self, cmd: String, out: String, code: Option<i32>) {
        // 서버가 못 하는 것이라고 답했으면 그 말을 우리말로 바꿔 준다(오케스트레이터는 키만 준다).
        let out = crate::errkey::human(self.lang, &out).replace("[exec.truncated]", tr(self.lang, "rcmd.truncated"));
        self.rcmd_result = Some(CmdResult { cmd, out, code, running: false });
    }

    /// 결과 창 — 읽기 전용. 목록 새로고침은 사용자가 정한다(명령이 파일을 바꿨을 수 있다).
    pub(crate) fn show_remote_cmd_result(&mut self, ctx: &egui::Context) {
        let Some(r) = self.rcmd_result.clone() else { return };
        let lang = self.lang;
        let mut open = true;
        let (mut copy, mut refresh) = (false, false);
        egui::Window::new(tr(lang, "rcmd.result"))
            .open(&mut open)
            .default_size([680.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&r.cmd).monospace().weak()).wrap());
                ui.separator();
                if r.running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr(lang, "rcmd.running"));
                    });
                } else {
                    ui.horizontal(|ui| {
                        match r.code {
                            Some(0) => ui.colored_label(crate::theme_ui::OK, format!("\u{2713} {}", tr(lang, "rcmd.ok"))),
                            Some(c) => ui.colored_label(crate::theme_ui::ERR, format!("\u{2717} exit {c}")),
                            // 코드를 못 받는 서버가 있다 — 없는 것을 0으로 꾸미지 않는다.
                            None => ui.weak(tr(lang, "rcmd.nocode")),
                        };
                        if ui.button(tr(lang, "findall.copy")).clicked() {
                            copy = true;
                        }
                        if ui.button(tr(lang, "sftp.refresh")).clicked() {
                            refresh = true;
                        }
                    });
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&r.out).monospace()).wrap());
                });
            });
        if copy {
            ctx.copy_text(r.out.clone());
        }
        if refresh {
            if let Some(id) = self.sftp.id {
                let path = self.sftp.path.clone();
                self.remote_nav(id, path);
            }
        }
        if !open {
            self.rcmd_result = None;
        }
    }

    /// 우클릭 메뉴에 명령 목록을 그린다. 고르면 확인창으로 넘어간다.
    pub(crate) fn remote_cmd_menu(ui: &mut egui::Ui, lang: nabi_i18n::Lang) -> Option<RemoteOp> {
        let mut picked = None;
        ui.menu_button(tr(lang, "rcmd.menu"), |ui| {
            ui.weak(tr(lang, "rcmd.posix"));
            ui.separator();
            for op in OPS {
                if ui.button(tr(lang, op.key)).clicked() {
                    picked = Some(*op);
                    ui.close();
                }
            }
        });
        picked
    }
}
