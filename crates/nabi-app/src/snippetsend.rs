//! 스니펫 전송 — 변수 치환 + 대화형 인자({{?:라벨}}) 입력 모달(D6). 메뉴·팔레트 공용.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    /// 스니펫을 포커스 pane에 전송한다. 대화형 인자({{?}}/{{?:라벨}})가 있으면 입력 모달을 띄우고,
    /// 없으면 즉시 치환·전송한다(메뉴·팔레트 공용 SSOT).
    pub(crate) fn send_snippet(&mut self, cmd: &str) {
        let Some(p) = self.focused_pane() else { return };
        let ph = crate::snippetvars::placeholders(cmd);
        if ph.is_empty() {
            self.dispatch_snippet(p, cmd, &[]);
        } else {
            self.snippet_prompt = Some((p, cmd.to_string(), ph.into_iter().map(|k| (k, String::new())).collect()));
        }
    }

    /// 변수 치환 후 pane에 전송한다(extra=대화형 인자 [(키,값)]). 키는 placeholders가 준 "?:라벨".
    fn dispatch_snippet(&mut self, p: PaneId, cmd: &str, extra: &[(String, String)]) {
        let clip = crate::paneio::clipboard_text().unwrap_or_default();
        let mut vars: Vec<(&str, String)> = self.snippet_vars(p); // 공변성으로 &'static→&'a 허용.
        for (k, v) in extra {
            vars.push((k.as_str(), v.clone()));
        }
        let mut data = crate::snippetvars::expand(cmd, &clip, &vars).into_bytes();
        data.push(b'\r');
        self.orch.send(nabi_proto::Command::WriteInput { pane: p, data: bytes::Bytes::from(data) });
    }

    /// 대화형 인자 입력 모달 — 각 {{?:라벨}}의 값을 받아 전송한다.
    pub(crate) fn snippet_prompt_modal(&mut self, ctx: &egui::Context) {
        let Some((pane, cmd, mut fields)) = self.snippet_prompt.take() else { return };
        let lang = self.lang;
        let (mut send, mut cancel) = (false, false);
        // 분리 창 위로(공통 Foreground 모달). X 대신 취소 버튼/Esc.
        crate::modal::foreground_modal(ctx, "snippet_prompt", |ui| {
            ui.heading(tr(lang, "snippets.fill"));
            for (key, val) in fields.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(key.strip_prefix("?:").unwrap_or("?"));
                    ui.text_edit_singleline(val);
                });
            }
            ui.horizontal(|ui| {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.button(tr(lang, "snippets.send")).clicked() { send = true; }
                if ui.button(tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { cancel = true; }
        });
        if send {
            self.dispatch_snippet(pane, &cmd, &fields);
        } else if !cancel {
            self.snippet_prompt = Some((pane, cmd, fields)); // 입력 보존(전송/취소 전까지).
        }
    }
}
