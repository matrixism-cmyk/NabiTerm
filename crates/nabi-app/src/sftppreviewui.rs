//! 원격 파일 미리보기 창.
//!
//! 앞부분만 받아 보여 준다. 무엇을 보여 줄지 정하는 규칙은 전부 `sftppreview`의 순수
//! 함수에 있고, 여기서는 받은 것을 그린다.

use crate::app::NabiApp;
use crate::sftppreview::Preview;
use nabi_i18n::tr;

/// 받아 올 최대 바이트. 텍스트 설정 파일이면 이 안에서 거의 다 보인다.
pub(crate) const MAX: usize = 64 * 1024;

/// 창이 들고 있는 것.
pub(crate) struct PreviewState {
    pub path: String,
    /// 아직 안 왔으면 None.
    pub result: Option<Result<Preview, String>>,
}

impl NabiApp {
    /// 미리보기를 요청한다(원격에 물어보고 결과를 기다린다).
    pub(crate) fn request_preview(&mut self, path: String) {
        let Some(id) = self.sftp.id else { return };
        self.preview = Some(PreviewState { path: path.clone(), result: None });
        self.orch.send(nabi_proto::Command::SftpPreview { id, path, max: MAX });
    }

    /// 결과가 도착했다.
    pub(crate) fn preview_arrived(&mut self, path: String, data: Vec<u8>, more: bool, err: Option<String>) {
        let Some(st) = self.preview.as_mut() else { return };
        // 사용자가 그새 다른 파일을 눌렀을 수 있다 — 늦게 온 답이 새 요청을 덮으면 안 된다.
        if st.path != path {
            return;
        }
        st.result = Some(match err {
            Some(e) => Err(e),
            None => Ok(crate::sftppreview::describe(&data, more)),
        });
    }

    /// 열려 있으면 그린다.
    pub(crate) fn show_preview(&mut self, ctx: &egui::Context) {
        let Some(st) = self.preview.as_ref() else { return };
        let lang = self.lang;
        let mut open = true;
        let mut copy: Option<String> = None;
        // 미리보기는 "확인"이고, 확인 다음은 대개 "고치기"나 "가져오기"다. 창을 닫고 다시
        // 목록에서 찾게 하면 그 사이가 끊긴다.
        let mut go_edit = false;
        let mut go_download = false;
        egui::Window::new(format!("{} — {}", tr(lang, "sftp.preview"), st.path))
            .open(&mut open)
            .default_size([760.0, 520.0])
            .collapsible(false)
            .show(ctx, |ui| match &st.result {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr(lang, "sftp.preview.loading"));
                    });
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::from_rgb(0xd0, 0x4a, 0x3a), e);
                }
                Some(Ok(p)) => {
                    body(ui, lang, p, &mut copy);
                    ui.separator();
                    ui.horizontal(|ui| {
                        go_edit = ui.button(tr(lang, "sftp.edit")).clicked();
                        go_download = ui.button(tr(lang, "sftp.preview.download")).clicked();
                    });
                }
            });
        if let Some(c) = copy {
            ctx.copy_text(c);
        }
        // 이어가기 — 원격 경로에서 이름만 떼어 기존 경로를 그대로 탄다(별도 길을 내지 않는다).
        let name = st.path.rsplit('/').next().unwrap_or_default().to_string();
        if go_edit {
            self.preview = None;
            self.edit_remote_dispatch(name);
            return;
        }
        if go_download {
            self.preview = None;
            // 목록에서 누른 것과 **같은 길**을 탄다(목적지 묻기·다중 선택 규칙 그대로).
            if let Some(id) = self.sftp.id {
                let size = self.sftp.entries.iter().find(|e| e.name == name).map(|e| e.size).unwrap_or(0);
                self.download_prompt(id, vec![(name, size)]);
            }
            return;
        }
        if !open {
            self.preview = None;
        }
    }
}

/// 결과 본문.
fn body(ui: &mut egui::Ui, lang: nabi_i18n::Lang, p: &Preview, copy: &mut Option<String>) {
    match p {
        Preview::Empty => {
            ui.weak(tr(lang, "sftp.preview.empty"));
        }
        Preview::Text { body, encoding, truncated } => {
            ui.horizontal(|ui| {
                ui.weak(encoding); // 깨져 보이면 이게 첫 실마리다.
                if *truncated {
                    ui.weak(tr(lang, "sftp.preview.partial"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(tr(lang, "menu.copy")).clicked() {
                        *copy = Some(body.clone());
                    }
                });
            });
            ui.separator();
            scroll(ui, "prev_text", body);
        }
        Preview::Binary { hex, shown } => {
            ui.horizontal(|ui| {
                ui.weak(tr(lang, "sftp.preview.binary"));
                ui.weak(format!("{shown} B"));
            });
            ui.separator();
            scroll(ui, "prev_hex", hex);
        }
    }
}

/// 넓은 줄이 창을 밀어내지 않게 가로도 함께 스크롤한다.
fn scroll(ui: &mut egui::Ui, salt: &str, text: &str) {
    egui::ScrollArea::both().id_salt(salt).auto_shrink([false, false]).show(ui, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap_mode(egui::TextWrapMode::Extend));
    });
}
