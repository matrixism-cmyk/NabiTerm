//! **공개키를 서버에 설치**하는 화면 — 명령 만들기·중복 검사는 `nabi_ssh::copyid`가 한다.
//!
//! 흐름은 두 걸음이다. 서버의 `authorized_keys`를 **먼저 읽고**, 그 안에 없을 때만 넣는다.
//! 한 걸음으로 줄일 수도 있지만(`grep -q || echo >>`), 그러면 사용자에게 "이미 있었다"와
//! "새로 넣었다"를 구별해 말해 줄 수 없다.
//!
//! ## 왜 SFTP 연결을 빌려 쓰나
//!
//! 배치 O에서 만든 exec 통로(`SftpExec`)를 그대로 쓴다. 붙어 있는 SFTP 세션의 SSH 연결에
//! 채널 하나를 더 여는 것이라, **새 연결도 새 인증도 필요 없다.** 키를 넣으려고 비밀번호를
//! 또 묻는 일이 없다.
//!
//! ## 무엇을 보여 주고 무엇을 묻나
//!
//! 남의 서버 파일을 고치는 일이므로 실행 전에 **명령 전문**을 보여 준다(원격 명령과 같은
//! 규칙). 다만 `authorized_keys`를 읽는 것은 보여 주지 않는다 — 읽기만 하고 아무것도
//! 바꾸지 않으므로, 그것까지 물으면 확인창이 습관이 된다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

/// 설치 흐름의 지금 단계.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// 서버의 현재 키 목록을 읽는 중.
    Reading,
    /// 읽었고, 넣어도 되는지 사용자에게 묻는 중(명령 전문을 보여 준다).
    Confirm(String),
    /// 넣는 중.
    Installing,
}

/// 진행 중인 설치 한 건.
#[derive(Clone)]
pub(crate) struct CopyId {
    pub id: nabi_proto::SftpId,
    /// 넣으려는 공개키 한 줄.
    pub key: String,
    /// 어느 키인지 사람에게 보일 이름(파일 이름 또는 주석).
    pub label: String,
    pub step: Step,
}

impl NabiApp {
    /// 파일에서 공개키를 골라 설치를 시작한다.
    pub(crate) fn start_copy_id(&mut self) {
        let Some(id) = self.sftp.id else {
            self.notify = Some((tr(self.lang, "copyid.needsftp").to_string(), Instant::now()));
            return;
        };
        let dir = crate::browser::home_dir().join(".ssh");
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&dir)
            .add_filter("public key", &["pub"])
            .pick_file()
        else {
            return;
        };
        let Ok(key) = std::fs::read_to_string(&path) else {
            self.notify = Some((tr(self.lang, "copyid.badkey").to_string(), Instant::now()));
            return;
        };
        let key = key.trim().to_string();
        // 공개키가 아닌 것을 넣으면 서버 파일만 더럽힌다 — 넣기 전에 거른다.
        if nabi_ssh::copyid::key_ident(&key).is_none() {
            self.notify = Some((tr(self.lang, "copyid.badkey").to_string(), Instant::now()));
            return;
        }
        let label = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.copy_id = Some(CopyId { id, key, label, step: Step::Reading });
        // 먼저 읽는다. 넣을지 말지는 그 답을 보고 정한다.
        let cmd = nabi_ssh::copyid::read_command().to_string();
        self.orch.send(nabi_proto::Command::SftpExec { id, cmd });
    }

    /// exec 결과가 왔다. 이 설치 흐름의 것이면 처리하고 true.
    pub(crate) fn on_copy_id_reply(&mut self, out: &str, code: Option<i32>) -> bool {
        let Some(mut c) = self.copy_id.clone() else { return false };
        match c.step {
            Step::Reading => {
                if nabi_ssh::copyid::already_present(out, &c.key) {
                    // 두 줄이 되면 나중에 지울 때 헷갈린다 — 있으면 그대로 둔다.
                    let msg = format!("{} ({})", tr(self.lang, "copyid.already"), c.label);
                    self.notify = Some((msg, Instant::now()));
                    self.copy_id = None;
                    return true;
                }
                c.step = Step::Confirm(nabi_ssh::copyid::install_command(&c.key));
                self.copy_id = Some(c);
                true
            }
            Step::Installing => {
                // 표시가 왔고 종료 코드도 0이어야 성공이다. 하나만 보면 반쪽이다 —
                // 표시는 앞 단계에서 나올 수 있고, 코드는 서버가 안 줄 수도 있다.
                let ok = out.contains(nabi_ssh::copyid::OK_MARK) && code.unwrap_or(0) == 0;
                let key = if ok { "copyid.done" } else { "copyid.failed" };
                let mut msg = format!("{} ({})", tr(self.lang, key), c.label);
                if !ok && !out.trim().is_empty() {
                    msg.push_str(&format!(" \u{b7} {}", out.trim()));
                }
                self.notify = Some((msg, Instant::now()));
                self.copy_id = None;
                true
            }
            Step::Confirm(_) => false, // 확인 중에는 우리 것이 아니다.
        }
    }

    /// 확인창 — 무엇이 실행되는지 글자 그대로 보여 준다.
    pub(crate) fn show_copy_id(&mut self, ctx: &egui::Context) {
        let Some(c) = self.copy_id.clone() else { return };
        let Step::Confirm(cmd) = c.step.clone() else { return };
        let lang = self.lang;
        let (mut run, mut cancel) = (false, false);
        crate::modal::foreground_modal(ctx, "copyid_confirm", |ui| {
            ui.heading(tr(lang, "copyid.title"));
            ui.label(format!("{}: {}", tr(lang, "copyid.key"), c.label));
            ui.label(format!("{}: {}", tr(lang, "hostkey.host"), self.sftp.host));
            ui.add_space(6.0);
            ui.label(tr(lang, "copyid.willrun"));
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&cmd).monospace()).wrap());
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "copyid.install")).clicked() {
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
            self.copy_id = None;
        }
        if run {
            let mut c2 = c;
            c2.step = Step::Installing;
            let id = c2.id;
            self.copy_id = Some(c2);
            self.orch.send(nabi_proto::Command::SftpExec { id, cmd });
        }
    }
}
