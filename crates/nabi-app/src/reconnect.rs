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
                cwd: None, is_ftp: false, open_sftp: false, tag: Default::default(),
            });
        }
    }

    /// 죽은 pane을 정리하고 같은 출처(pane_origins)로 재연결한다. 모달 [다시 연결]과
    /// 자동 재접속(P7)이 공유하는 SSOT. 볼트 잠금 시 connect_saved가 Quick Connect 프리필 폴백.
    pub(crate) fn do_reconnect(&mut self, pane: nabi_types::PaneId) {
        if let Some(kind) = self.pane_origins.get(&pane).cloned() {
            self.orch.send(nabi_proto::Command::ClosePane { pane });
            // 출처가 같은 **저장 세션을 되찾아** 그대로 쓴다. 예전에는 이름 없는 세션을
            // 새로 지어 붙였는데, 터널 규칙이 세션 이름을 열쇠로 살기 때문에 재접속이
            // 성공해도 터널이 돌아오지 않았다(접속 후 명령도 함께 사라졌다).
            let s = crate::reconnectsess::session_for(&self.sessions.sessions, &kind);
            self.connect_saved(s);
        }
    }
}

impl NabiApp {
    /// 자동 재접속을 시작한다(끊김 순간). 첫 시도는 잠깐 기다렸다 한다 —
    /// 끊기자마자 두드리면 아직 정리되지 않은 소켓에 부딪히기 쉽다.
    pub(crate) fn begin_auto_reconnect(
        &mut self,
        pane: nabi_types::PaneId,
        message: String,
        carry: Option<(crate::backoff::Backoff, String)>,
    ) {
        // 이어받은 횟수가 있으면 거기서 계속한다 — 재접속하면 pane 번호가 바뀌므로
        // 이어받지 않으면 매번 처음부터 세게 되고, 그러면 영영 멈추지 않는다.
        let b = carry.map(|(b, _)| b).unwrap_or_else(crate::backoff::Backoff::first);
        self.reconnecting.insert(pane, (b, std::time::Instant::now() + b.wait(), message));
    }

    /// 기다리던 재접속을 때가 되면 실행한다. 매 프레임 부른다.
    ///
    /// 화면을 계속 다시 그려야 하면 true — 남은 시간을 세는 표시가 멈추지 않게.
    pub(crate) fn tick_auto_reconnect(&mut self) -> bool {
        if self.reconnecting.is_empty() {
            return false;
        }
        let now = std::time::Instant::now();
        let due: Vec<nabi_types::PaneId> =
            self.reconnecting.iter().filter(|(_, (_, at, _))| *at <= now).map(|(p, _)| *p).collect();
        for pane in due {
            let Some((b, _, msg)) = self.reconnecting.remove(&pane) else { continue };
            if !b.may_retry() {
                // 다 썼다 — 사용자에게 넘긴다. 무한히 시도하면 되지 않는 이유를 영영 못 본다.
                self.reconnect_ask = Some((pane, msg));
                continue;
            }
            let next = b.attempted();
            self.notify = Some((
                format!("\u{21bb} {} {}/{}", nabi_i18n::tr(self.lang, "reconn.trying"), next.tries, crate::backoff::MAX_TRIES),
                now,
            ));
            self.do_reconnect(pane);
            // 붙었는지는 여기서 알 수 없다. 다시 끊기면 SshDisconnected가 와서 이어간다.
            self.reconnect_carry = Some((next, msg));
        }
        !self.reconnecting.is_empty()
    }

    /// 재접속을 그만둔다(사용자가 멈춤). 자동으로 도는 것에는 늘 멈춤이 있어야 한다.
    pub(crate) fn stop_auto_reconnect(&mut self, pane: nabi_types::PaneId) {
        self.reconnecting.remove(&pane);
        self.reconnect_carry = None;
    }

    /// 재접속 중인 pane이 있으면 (남은 초, 시도/최대).
    pub(crate) fn reconnect_status(&self) -> Option<(u64, u32, u32)> {
        let (_, (b, at, _)) = self.reconnecting.iter().next()?;
        let left = at.saturating_duration_since(std::time::Instant::now()).as_secs();
        Some((left, b.tries + 1, crate::backoff::MAX_TRIES))
    }
}

/// 재접속 중이라는 것과 **멈추는 길**을 함께 보여 준다.
///
/// 자동으로 도는 것에는 늘 멈춤이 있어야 한다. 사용자가 지금 그 서버에 붙고 싶지 않을 수도
/// 있고(자리를 옮겼다, 서버를 내렸다), 무엇보다 프로그램이 자기 뜻과 무관하게 계속 무언가
/// 하고 있는 것을 볼 수 있어야 한다.
pub(crate) fn reconnect_bar(app: &mut NabiApp, ui: &mut egui::Ui) {
    let Some((left, try_n, max)) = app.reconnect_status() else { return };
    let lang = app.lang;
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(format!("{} {try_n}/{max}", tr(lang, "reconn.trying")));
        if left > 0 {
            ui.weak(format!("{left}s"));
        }
        if ui.small_button(tr(lang, "reconn.stop")).clicked() {
            app.stop_all_reconnects();
        }
    });
}

impl NabiApp {
    /// 기다리는 재접속을 전부 그만둔다.
    pub(crate) fn stop_all_reconnects(&mut self) {
        let panes: Vec<nabi_types::PaneId> = self.reconnecting.keys().copied().collect();
        for p in panes {
            self.stop_auto_reconnect(p);
        }
        self.notify = Some((tr(self.lang, "reconn.stopped").to_string(), std::time::Instant::now()));
    }
}
