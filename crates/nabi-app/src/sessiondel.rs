//! 저장 세션 삭제 확인 — ✕가 ✏(편집) 바로 옆이라 잘못 누르기 쉽고, 세션에는 호스트·계정·
//! 자격증명 참조·메모가 함께 걸려 있어 되돌릴 수 없다. 그래서 한 번 묻는다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 삭제 확인 모달(대기 중일 때만). 분리 창 위에도 떠야 해서 foreground_modal을 쓴다.
    pub(crate) fn session_delete_modal(&mut self, ctx: &egui::Context) {
        let Some(name) = self.session_delete_ask.clone() else { return };
        let lang = self.lang;
        let (mut ok, mut cancel) = (false, false);
        crate::modal::foreground_modal(ctx, "session_del", |ui| {
            ui.heading(tr(lang, "sessions.delete"));
            ui.label(&name);
            // 무엇이 함께 사라지는지 알려 준다(고정·메모도 같이 지워진다).
            ui.weak(tr(lang, "sessions.delete.warn"));
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "sessions.delete")).clicked() {
                    ok = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if ok {
            self.delete_session_now(&name);
        }
        if ok || cancel {
            self.session_delete_ask = None;
        }
    }

    /// 실제 삭제 — 세션 + 그 세션에 걸린 고정·메모까지 함께 정리(고아 방지).
    fn delete_session_now(&mut self, name: &str) {
        self.sessions.remove(name);
        let ap = &mut self.config.appearance;
        let pinned = ap.pinned_sessions.iter().any(|p| p == name);
        let noted = ap.session_notes.remove(name).is_some();
        ap.pinned_sessions.retain(|p| p != name);
        if pinned || noted {
            self.save_config();
        }
        self.save_sessions();
    }
}
