//! SSH 끊김 재연결 제안 창 — 같은 출처(SessionKind)로 원클릭 재접속.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 연결 끊김 안내 + [다시 연결]/[닫기] 창. 재연결은 pane_origins의 출처를 재사용한다.
    pub(crate) fn show_reconnect(&mut self, ctx: &egui::Context) {
        let Some((pane, msg)) = self.reconnect_ask.clone() else {
            return;
        };
        let lang = self.lang;
        let (mut retry, mut dismiss) = (false, false);
        // 끊김 시 자동 등장 — 분리 창 위로(공통 Foreground 모달).
        crate::modal::foreground_modal(ctx, "reconnect_ask", |ui| {
            ui.heading(tr(lang, "reconn.title"));
            ui.label(&msg);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, "reconn.retry")).clicked() { retry = true; }
                if ui.button(tr(lang, "qc.cancel")).clicked() { dismiss = true; }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { dismiss = true; }
        });
        if retry {
            self.do_reconnect(pane);
            self.reconnect_ask = None;
        } else if dismiss {
            self.reconnect_ask = None;
        }
    }

    /// 포커스 pane과 같은 출처(SSH/로컬)로 새 탭을 연다 — 닫지 않는 "세션 복제"(MobaXterm식).
    pub(crate) fn duplicate_connection(&mut self) {
        let Some(p) = self.focused_pane() else { return };
        if let Some(kind) = self.pane_origins.get(&p).cloned() {
            self.connect_saved(nabi_session::SavedSession {
                name: String::new(), folder: None, kind, on_connect: None,
                cwd: None, is_ftp: false, open_sftp: false,
            });
        }
    }

    /// 죽은 pane을 정리하고 같은 출처(pane_origins)로 재연결한다. 모달 [다시 연결]과
    /// 자동 재접속(P7)이 공유하는 SSOT. 볼트 잠금 시 connect_saved가 Quick Connect 프리필 폴백.
    pub(crate) fn do_reconnect(&mut self, pane: nabi_types::PaneId) {
        if let Some(kind) = self.pane_origins.get(&pane).cloned() {
            self.orch.send(nabi_proto::Command::ClosePane { pane });
            self.connect_saved(nabi_session::SavedSession {
                name: String::new(), folder: None, kind, on_connect: None,
                cwd: None, is_ftp: false, open_sftp: false,
            });
        }
    }
}
