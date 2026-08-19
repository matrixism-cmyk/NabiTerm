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

    /// 붙여넣기 전에 확인을 받아야 하는가 — 개행(설정 연동) 또는 유니코드 속임.
    fn paste_needs_confirm(&self, text: &str) -> bool {
        let t = &self.config.terminal;
        nabi_render::paste::needs_confirm_any(t.warn_paste_newline, t.warn_paste_unicode, text)
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
        // 확인 판단은 공용 함수 하나로 — 입구마다 규칙이 다르면 안전장치가 아니다.
        if self.paste_needs_confirm(&text) {
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
        if self.paste_needs_confirm(&text) {
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

    /// 키보드 직접 붙여넣기(Event::Paste, 예: Ctrl+V)를 **확인이 필요한 내용이면** 가로챈다.
    /// 입력 위젯/팝업 포커스 중에는 양보한다.
    ///
    /// 판단은 `paste_needs_confirm` 하나로 한다. 예전엔 여기만 "개행이 있을 때"라는 **다른**
    /// 조건을 썼는데, 그 탓에 한 줄짜리 붙여넣기는 확인을 전혀 거치지 않았다 —
    /// 유니코드 속임(제로폭·방향 재정의)은 대부분 한 줄이라 정확히 이 구멍으로 새어 나갔다.
    /// 안전장치는 입구마다 조건이 같아야 안전장치다.
    pub(crate) fn intercept_keyboard_paste(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) || egui::Popup::is_any_open(ctx) {
            return;
        }
        let risky = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Paste(s) if self.paste_needs_confirm(s)))
        });
        if !risky {
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

    /// 붙여넣기 확인 대화상자 — 줄 수·미리보기 + **유니코드 속임 경고**(있으면).
    ///
    /// 속임이 있으면 "위험 문자 제거 후 붙여넣기"를 함께 준다. 자동으로 지울 수 있는 것은
    /// 보이지 않는 문자뿐이라(호모글리프는 판단 불가) 제거 후에도 경고 문구는 남겨 둔다.
    pub(crate) fn show_paste_confirm(&mut self, ctx: &egui::Context) {
        let Some((pane, data)) = self.pending_paste.clone() else {
            return;
        };
        let lang = self.lang;
        let lines = data.iter().filter(|&&b| b == b'\n').count() + 1;
        let clean = String::from_utf8_lossy(&data).replace("\x1b[200~", "").replace("\x1b[201~", "");
        let risks = nabi_render::pastedeceive::scan(&clean);
        let (mut paste, mut strip, mut cancel) = (false, false, false);
        // 분리 창 위로 확실히 띄운다 — 공통 Foreground 모달(z-order 일관화).
        crate::modal::foreground_modal(ctx, "paste_confirm", |ui| {
            // 한 줄인데 "여러 줄" 문구가 나오면 안 된다 — 속임 문자 경고는 한 줄에서도 뜬다.
            ui.heading(nabi_i18n::tr(lang, if lines > 1 { "paste.title" } else { "paste.title.one" }));
            if lines > 1 {
                ui.label(format!("{} ({} lines)", nabi_i18n::tr(lang, "paste.body"), lines));
            }
            if !risks.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(crate::theme_ui::ERR, format!("\u{26a0} {}", nabi_i18n::tr(lang, "paste.risk.title")));
                for r in &risks {
                    ui.label(format!("\u{2022} {}", nabi_i18n::tr(lang, r.key())));
                }
                ui.add_space(4.0);
            }
            let preview: String = clean.lines().next().unwrap_or("").chars().take(60).collect();
            ui.monospace(preview);
            ui.horizontal(|ui| {
                if ui.button(nabi_i18n::tr(lang, "paste.do")).clicked() { paste = true; }
                if !risks.is_empty() && ui.button(nabi_i18n::tr(lang, "paste.strip")).clicked() { strip = true; }
                if ui.button(nabi_i18n::tr(lang, "qc.cancel")).clicked() { cancel = true; }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { cancel = true; }
        });
        if paste || strip {
            // 제거를 골랐으면 보이지 않는 문자만 걷어낸 바이트로 보낸다.
            let out = if strip { strip_invisible(&data) } else { data };
            self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(out) });
            self.pending_paste = None;
        } else if cancel {
            self.pending_paste = None;
        }
    }
}

/// 붙여넣기 바이트에서 보이지 않는 문자(방향 재정의·제로폭)만 제거한다.
/// bracketed 마커는 ASCII라 그대로 살아남는다.
fn strip_invisible(data: &[u8]) -> Vec<u8> {
    nabi_render::pastedeceive::strip(&String::from_utf8_lossy(data)).into_bytes()
}
