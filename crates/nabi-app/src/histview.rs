//! 휠을 올리면 **그 자리에서** 전체 기록을 보여 주는 겹 화면.
//!
//! ## 왜 필요한가
//!
//! 클로드 코드 같은 프로그램은 화면을 덮어 그려서 스크롤백에 조각만 남는다. 그래서
//! 휠을 올려도 볼 것이 없었다. "탭 우클릭 ▸ 전체 기록 열기"를 만들었지만, 사용자의
//! 요구는 분명했다 — **휠을 굴리면 바로 이전 내용이 보여야 한다**(2026-08-29).
//!
//! 그래서 그 상태(마우스를 가져간 프로그램 + 주 화면)에서 휠을 올리면, pane 위에
//! 전체 기록을 겹쳐 띄운다. 기록은 세션 기록(.cast)을 그때 펴 낸 것이다 —
//! 스크롤백에 안 올라간 내용까지 전부 있다.
//!
//! ## 동작
//!
//! * 열리면 맨 아래(가장 최근)에서 시작한다 — 스크롤백을 보는 것과 같은 방향.
//! * 휠은 이 겹 화면 안을 굴린다(egui 가 알아서 받는다).
//! * Esc·닫기 단추로 닫는다. 새로고침은 기록을 다시 편다(그 사이에 온 것 포함).
//! * "편집기로"는 nabiPad 로 넘긴다 — 검색하거나 저장하고 싶을 때.

use nabi_i18n::tr;
use nabi_types::PaneId;

/// 떠 있는 기록 겹 화면 하나.
pub(crate) struct HistView {
    pub pane: PaneId,
    pub lines: Vec<String>,
}

impl crate::app::NabiApp {
    /// 이 pane 의 기록 겹 화면을 연다(이미 떠 있으면 그대로 둔다).
    pub(crate) fn open_history_view(&mut self, pane: PaneId) {
        if self.hist_view.as_ref().is_some_and(|h| h.pane == pane) {
            return;
        }
        match self.flatten_pane_history(pane) {
            Ok(lines) => self.hist_view = Some(HistView { pane, lines }),
            Err(msg) => {
                // 기록이 없다 — **이제부터라도 남긴다.** 그리고 그때까지의 스크롤백이라도
                // 보여 준다. 빈손으로 안내만 하면 "휠이 안 된다"와 다를 게 없다.
                self.autolog_now(pane, "local");
                let dump = self
                    .orch
                    .panes
                    .read()
                    .ok()
                    .and_then(|m| m.get(&pane).cloned())
                    .and_then(|v| v.model.lock().ok().map(|md| md.dump_text(100_000)))
                    .unwrap_or_default();
                match dump.trim().is_empty() {
                    true => self.notify = Some((msg, std::time::Instant::now())),
                    false => {
                        self.hist_view = Some(HistView {
                            pane,
                            lines: dump.lines().map(str::to_string).collect(),
                        });
                        // 겹 화면과 함께 사정도 알린다 — 다음번에는 전체가 남는다.
                        self.notify = Some((msg, std::time::Instant::now()));
                    }
                }
            }
        }
    }

    /// 세션 기록을 읽어 줄 목록으로 편다.
    pub(crate) fn flatten_pane_history(&mut self, pane: PaneId) -> Result<Vec<String>, String> {
        // 방금까지의 것도 보여야 한다 — 안 흘려보내면 몇 초 전 것이 빠진다.
        self.flush_session_logs();
        let Some(src) = self.session_logs.get(&pane).map(|l| l.path.clone()) else {
            return Err(tr(self.lang, "hist.notrecording").to_string());
        };
        let text = std::fs::read_to_string(&src).map_err(|e| format!("\u{2715} {e}"))?;
        let plain = crate::castplain::cast_to_plain(&text);
        match plain.trim().is_empty() {
            true => Err(tr(self.lang, "hist.empty").to_string()),
            false => Ok(plain.lines().map(str::to_string).collect()),
        }
    }

    /// 겹 화면을 그린다. 매 프레임 부른다 — 닫혀 있으면 아무 일도 없다.
    pub(crate) fn render_history_view(&mut self, ctx: &egui::Context) {
        let Some(h) = &self.hist_view else { return };
        let pane = h.pane;
        // pane 이 닫혔거나 자리를 모르면 함께 닫는다 — 허공에 떠 있을 이유가 없다.
        let Some(rect) = self.pane_rects.get(&pane).copied() else {
            self.hist_view = None;
            return;
        };
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut refresh = false;
        let mut to_editor = false;
        let n = h.lines.len();
        egui::Area::new(egui::Id::new(("hist_view", pane)))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                egui::Frame::window(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("\u{1f4dc} {} \u{00b7} {n}", tr(self.lang, "hist.title")));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            close |= ui.button("\u{2715}").clicked();
                            to_editor = ui.button(tr(self.lang, "hist.toeditor")).clicked();
                            refresh = ui.button("\u{21bb}").clicked();
                        });
                    });
                    ui.separator();
                    let row = ui.text_style_height(&egui::TextStyle::Monospace);
                    egui::ScrollArea::vertical()
                        .max_height(rect.height() - 60.0)
                        .stick_to_bottom(true) // 열리면 맨 아래(최근)부터 — 위로 굴리면 풀린다.
                        .show_rows(ui, row, n, |ui, range| {
                            let Some(h) = &self.hist_view else { return };
                            for line in &h.lines[range] {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(line).monospace())
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                );
                            }
                        });
                });
            });
        if refresh {
            if let Ok(lines) = self.flatten_pane_history(pane) {
                if let Some(h) = &mut self.hist_view {
                    h.lines = lines;
                }
            }
        }
        if to_editor {
            self.open_pane_history(pane); // nabiPad 로 — 검색·저장은 그쪽이 낫다.
            close = true;
        }
        if close {
            self.hist_view = None;
        }
    }
}
