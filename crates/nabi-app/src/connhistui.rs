//! 접속 이력의 기록 훅과 보기 창(순수 로직은 `connhist`).
//!
//! 기록은 pane의 **출처**(`pane_origins`)에서 호스트·사용자를 꺼내 온다. 화면 제목을 쓰면
//! 사용자가 제목을 바꾸는 순간 이력이 다른 서버 것처럼 보인다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 지금 unix 초.
fn now() -> i64 {
    chrono::Local::now().timestamp()
}

impl NabiApp {
    /// 이 pane이 붙었다 — 이력을 연다.
    pub(crate) fn note_connection_open(&mut self, pane: nabi_types::PaneId) {
        if !self.config.terminal.keep_conn_history {
            return;
        }
        let Some(nabi_session::SessionKind::Ssh { host, user, .. }) = self.pane_origins.get(&pane).cloned() else {
            return;
        };
        // 이름은 저장 세션에서 되찾는다(재접속과 같은 통로 — 이름이 있어야 목록에서 읽힌다).
        let name = self
            .pane_origins
            .get(&pane)
            .and_then(|k| crate::reconnectsess::pick(&self.sessions.sessions, k))
            .map(|s| s.name.clone())
            .unwrap_or_default();
        crate::connhist::note_open(
            &mut self.conn_hist,
            crate::connhist::Entry { name, host, user, at: now(), secs: None, why: String::new() },
        );
        self.save_conn_hist();
    }

    /// 이 pane이 끊겼다 — 열려 있던 이력을 닫는다.
    pub(crate) fn note_connection_close(&mut self, pane: nabi_types::PaneId, why: &str) {
        if !self.config.terminal.keep_conn_history {
            return;
        }
        let Some(nabi_session::SessionKind::Ssh { host, user, .. }) = self.pane_origins.get(&pane).cloned() else {
            return;
        };
        if crate::connhist::note_close(&mut self.conn_hist, &host, &user, now(), why) {
            self.save_conn_hist();
        }
    }

    fn save_conn_hist(&self) {
        crate::connhist::save(&nabi_config::resolve_base(), &self.conn_hist);
    }

    /// 이력 창을 연다(팔레트·도구 메뉴).
    pub(crate) fn open_conn_history(&mut self) {
        self.conn_hist_open = true;
    }

    /// 이력 창.
    pub(crate) fn show_conn_history(&mut self, ctx: &egui::Context) {
        if !self.conn_hist_open {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        let mut clear = false;
        egui::Window::new(tr(lang, "connhist.title"))
            .open(&mut open)
            .default_size([620.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {}", tr(lang, "connhist.count"), self.conn_hist.len()));
                    if !self.conn_hist.is_empty() && ui.button(tr(lang, "connhist.clear")).clicked() {
                        clear = true;
                    }
                });
                ui.weak(tr(lang, "connhist.what"));
                ui.separator();
                if self.conn_hist.is_empty() {
                    ui.weak(tr(lang, "connhist.none"));
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for e in &self.conn_hist {
                        let when = chrono::DateTime::from_timestamp(e.at, 0)
                            .map(|t| t.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
                            .unwrap_or_default();
                        // 아직 붙어 있는 것은 시간 대신 표시를 둔다 — 0초로 보이면 안 된다.
                        let dur = match e.secs {
                            Some(s) => crate::connhist::human_secs(s),
                            None => tr(lang, "connhist.open").to_string(),
                        };
                        let who = format!("{}@{}", e.user, e.host);
                        let label = match e.name.is_empty() {
                            true => format!("{when}  {who}  {dur}"),
                            false => format!("{when}  {}  ({who})  {dur}", e.name),
                        };
                        let r = ui.add(egui::Label::new(egui::RichText::new(label).monospace()));
                        if !e.why.is_empty() {
                            r.on_hover_text(&e.why);
                        }
                    }
                });
            });
        self.conn_hist_open = open;
        if clear {
            self.conn_hist.clear();
            self.save_conn_hist();
        }
    }
}
