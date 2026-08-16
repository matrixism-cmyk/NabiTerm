//! 붙여넣기 처리 — 클립보드 → 포커스 pane, 여러 줄 붙여넣기 안전 확인(tabops에서 분리).

use crate::app::NabiApp;

impl NabiApp {
    /// 복사된 텍스트를 클립보드 히스토리에 기록한다(최신 우선·중복 제거·30개 상한, 메모리 전용=프라이버시).
    pub(crate) fn record_clip(&mut self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.clip_history.retain(|t| t != text);
        self.clip_history.insert(0, text.to_string());
        self.clip_history.truncate(30);
    }

    /// pane이 bracketed paste 모드인가(모르면 false).
    fn pane_bracketed(&self, pane: nabi_types::PaneId) -> bool {
        self.orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&pane).cloned())
            .and_then(|v| v.model.lock().ok().map(|md| md.bracketed_paste()))
            .unwrap_or(false)
    }

    /// 주어진 텍스트를 포커스 pane에 붙여넣는다 — **모든 붙여넣기가 지나는 한 길**.
    ///
    /// 여러 줄 확인이 여기 있어야 한다. 예전에는 Ctrl+Shift+V만 확인을 거치고, 클립보드
    /// 히스토리에서 고른 텍스트는 곧장 셸로 들어갔다 — 같은 위험(여러 줄이면 즉시 실행)인데
    /// 한쪽만 막고 있었다.
    pub(crate) fn paste_text_to_focused(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        let Some(pane) = self.focused_pane() else { return };
        let data = crate::paneio::wrap_paste(&text, self.pane_bracketed(pane));
        // 여러 줄 붙여넣기는 설정 시 확인 대화상자로 미룬다(붙여넣기 안전).
        if self.config.terminal.warn_paste_newline && text.contains('\n') {
            self.pending_paste = Some((pane, data));
            return;
        }
        self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
    }

    /// **지정한 pane**에 텍스트를 붙여넣는다 — 마우스 붙여넣기·분리 창이 쓰는 입구.
    ///
    /// 포커스가 아니라 pane을 직접 받는다. 분리 창이나 분할에서 우클릭한 pane은 "포커스된
    /// pane"과 다를 수 있어서다. 확인 판단은 여기서도 같은 규칙을 쓴다 — 안전장치가
    /// 입구마다 다르면 없는 것과 같다(우클릭 붙여넣기가 그렇게 비껴가고 있었다).
    pub(crate) fn paste_text_to_pane(&mut self, pane: nabi_types::PaneId, text: String) {
        if text.is_empty() {
            return;
        }
        let data = crate::paneio::wrap_paste(&text, self.pane_bracketed(pane));
        if nabi_render::paste::needs_paste_confirm(self.config.terminal.warn_paste_newline, &text) {
            self.pending_paste = Some((pane, data));
            return;
        }
        self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
    }

    /// 클립보드를 포커스된 pane에 붙여넣는다(Ctrl+Shift+V; bracketed 모드면 래핑).
    pub(crate) fn paste_to_focused(&mut self) {
        let Some(text) = crate::paneio::clipboard_text() else {
            return;
        };
        self.paste_text_to_focused(text);
    }

    /// 키보드 직접 붙여넣기(Event::Paste, 예: Ctrl+V)도 여러 줄이고 경고 설정이 켜져 있으면
    /// 터미널로 새기 전에 가로채 확인 경로로 보낸다. 입력 위젯/팝업 포커스 중에는 양보한다.
    pub(crate) fn intercept_keyboard_paste(&mut self, ctx: &egui::Context) {
        if !self.config.terminal.warn_paste_newline
            || ctx.memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ctx)
        {
            return;
        }
        let multiline = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Paste(s) if s.contains('\n')))
        });
        if !multiline {
            return;
        }
        // 포커스가 터미널 pane일 때만(원격 SFTP 패널 제외).
        let Some(p) = self.focused_pane() else {
            return;
        };
        if Some(p) == self.sftp_pane || self.sftp_bg.contains_key(&p) {
            return;
        }
        // Paste 이벤트를 제거해 events_to_bytes가 즉시 보내지 못하게 하고 확인 경로로.
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
        self.paste_to_focused();
    }

    /// 여러 줄 붙여넣기 확인 대화상자(Paste/Cancel·Esc 취소).
    pub(crate) fn show_paste_confirm(&mut self, ctx: &egui::Context) {
        let Some((pane, data)) = self.pending_paste.clone() else {
            return;
        };
        let lang = self.lang;
        let lines = data.iter().filter(|&&b| b == b'\n').count() + 1;
        let mut paste = false;
        let mut cancel = false;
        // 분리 창 위로 확실히 띄운다 — 공통 Foreground 모달(z-order 일관화).
        crate::modal::foreground_modal(ctx, "paste_confirm", |ui| {
            ui.heading(nabi_i18n::tr(lang, "paste.title"));
            ui.label(format!("{} ({} lines)", nabi_i18n::tr(lang, "paste.body"), lines));
            let clean = String::from_utf8_lossy(&data).replace("\x1b[200~", "").replace("\x1b[201~", "");
            let preview: String = clean.lines().next().unwrap_or("").chars().take(60).collect();
            ui.monospace(preview);
            ui.horizontal(|ui| {
                if ui.button(nabi_i18n::tr(lang, "paste.do")).clicked() { paste = true; }
                if ui.button(nabi_i18n::tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { cancel = true; }
        });
        if paste {
            self.orch.send(nabi_proto::Command::WriteInput {
                pane,
                data: bytes::Bytes::from(data),
            });
            self.pending_paste = None;
        } else if cancel {
            self.pending_paste = None;
        }
    }
}
