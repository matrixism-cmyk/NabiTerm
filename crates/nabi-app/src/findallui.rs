//! "모든 창에서 찾기" 창 — findall이 모은 결과를 보여 주고 그 자리로 보낸다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// '모든 창에서 찾기' 창의 상태. 창을 닫으면 통째로 버린다(이력만 다음에 다시 쌓인다).
#[derive(Clone, Default)]
pub(crate) struct FindAll {
    pub query: String,
    pub regex: bool,
    pub whole: bool,
    /// 맞는 줄을 **빼고** 본다 — 잡음 걷어내기.
    pub invert: bool,
    /// 지금 보고 있는 창만 훑는다(로그 하나를 걸러 볼 때가 대부분이다).
    pub this_pane_only: bool,
    pub results: Vec<crate::findall::Hit>,
    /// 상한에 걸려 결과를 다 담지 못했는가.
    pub truncated: bool,
    /// 최근 검색어(최신 우선).
    pub history: Vec<String>,
}

impl NabiApp {
    /// 창을 연다(팔레트·찾기 바에서). 이미 열려 있으면 검색어만 이어 쓴다.
    pub(crate) fn open_find_all(&mut self) {
        if self.find_all.is_none() {
            self.find_all = Some(Default::default());
        }
        // 지금 pane 검색어가 있으면 그대로 가져온다 — 다시 치게 만들지 않는다.
        if let (Some(st), false) = (self.find_all.as_mut(), self.find_query.is_empty()) {
            st.query = self.find_query.clone();
        }
    }

    /// 열려 있는 모든 터미널 pane을 훑어 결과를 채운다.
    fn run_find_all(&mut self) {
        let here = self.focused_pane();
        let Some(st) = self.find_all.as_mut() else { return };
        st.results.clear();
        st.truncated = false;
        let Some(m) = crate::find::build_matcher(&st.query, st.regex, st.whole) else { return };
        crate::findall::push_history(&mut st.history, &st.query);
        let limit = self.config.terminal.search_limit;
        let only = if st.this_pane_only { here } else { None };
        let invert = st.invert;
        let panes: Vec<_> = match only {
            Some(p) => vec![p],
            None => self.dock.iter_all_tabs().map(|(_, p)| *p).collect(),
        };
        let mut budget = crate::findall::TOTAL;
        let Ok(map) = self.orch.panes.read() else { return };
        for p in panes {
            if budget == 0 {
                st.truncated = true;
                break;
            }
            let Some(view) = map.get(&p) else { continue };
            let Ok(model) = view.model.lock() else { continue };
            let total = model.total_abs_lines();
            let from = total.saturating_sub(limit);
            let lines = model.lines_abs_text(from, total);
            let got = crate::findall::scan_pane(p, &view.title, &lines, from, &m, invert, budget);
            budget = budget.saturating_sub(got.hits.len());
            st.truncated |= got.more;
            st.results.extend(got.hits);
        }
    }

    /// 결과 창을 그린다. 줄을 누르면 그 pane으로 가서 그 줄로 스크롤한다.
    pub(crate) fn show_find_all(&mut self, ctx: &egui::Context) {
        if self.find_all.is_none() {
            return;
        }
        let lang = self.lang;
        let (mut open, mut search, mut goto, mut save) = (true, false, None, false);
        let st = self.find_all.clone().unwrap_or_default();
        let mut next = st.clone();
        egui::Window::new(tr(lang, "findall.title"))
            .open(&mut open)
            .default_size([760.0, 480.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut next.query)
                            .desired_width(320.0)
                            .hint_text(tr(lang, "findall.hint")),
                    );
                    if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        search = true;
                    }
                    if ui.button(tr(lang, "find.search")).clicked() {
                        search = true;
                    }
                    ui.checkbox(&mut next.regex, tr(lang, "find.regex"));
                    ui.checkbox(&mut next.whole, tr(lang, "find.whole"));
                    // 제외 — 찾기의 반대편 절반. 로그에서는 "이것만 빼고"가 자주 필요하다.
                    if ui.checkbox(&mut next.invert, tr(lang, "find.invert")).changed() {
                        search = true;
                    }
                    if ui.checkbox(&mut next.this_pane_only, tr(lang, "findall.thispane")).changed() {
                        search = true;
                    }
                });
                // 검색어 이력 — 같은 것을 다시 치게 만들지 않는다.
                if !st.history.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.weak(tr(lang, "findall.recent"));
                        for h in st.history.iter().take(8) {
                            if ui.small_button(h).clicked() {
                                next.query = h.clone();
                                search = true;
                            }
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {}", tr(lang, "findall.found"), st.results.len()));
                    // 걸러 낸 결과를 가져가는 길 — 안 그러면 다시 손으로 옮겨 적게 된다.
                    if !st.results.is_empty() {
                        if ui.button(tr(lang, "findall.copy")).clicked() {
                            ui.ctx().copy_text(crate::findall::to_text(&st.results));
                        }
                        if ui.button(tr(lang, "findall.save")).clicked() {
                            save = true;
                        }
                    }
                    if st.truncated {
                        // 상한에 걸렸으면 말해 준다 — 조용히 자르면 없는 것으로 오해한다.
                        ui.colored_label(crate::theme_ui::BROADCAST, tr(lang, "findall.truncated"));
                    }
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut last: Option<nabi_types::PaneId> = None;
                    for h in &st.results {
                        if last != Some(h.pane) {
                            ui.add_space(6.0);
                            ui.strong(&h.title);
                            last = Some(h.pane);
                        }
                        let label = format!("{:>7}  {}", h.abs_line + 1, h.text);
                        if ui.add(egui::Label::new(egui::RichText::new(label).monospace()).sense(egui::Sense::click())).clicked() {
                            goto = Some((h.pane, h.abs_line));
                        }
                    }
                });
            });
        self.find_all = open.then_some(next);
        if search {
            self.run_find_all();
        }
        if save {
            self.save_find_all(&st.results);
        }
        if let Some((pane, line)) = goto {
            self.jump_to_scrollback(pane, line);
        }
    }

    /// 걸러 낸 결과를 파일로 남긴다. 저장 대화상자는 기존 통로를 그대로 쓴다.
    fn save_find_all(&mut self, hits: &[crate::findall::Hit]) {
        let text = crate::findall::to_text(hits);
        let Some(path) = rfd::FileDialog::new().set_file_name("filtered.txt").add_filter("text", &["txt", "log"]).save_file() else {
            return;
        };
        let msg = match std::fs::write(&path, text) {
            Ok(()) => tr(self.lang, "findall.saved").to_string(),
            Err(e) => format!("{}: {e}", tr(self.lang, "findall.savefail")),
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }

    /// 그 pane을 앞으로 가져오고 해당 줄로 스크롤한다.
    ///
    /// 포커스는 기존 요청 통로(`focus_req`)를 쓴다 — dock 조작 시점이 프레임 안에서
    /// 정해져 있어서, 여기서 직접 만지면 그 규칙을 깬다.
    fn jump_to_scrollback(&mut self, pane: nabi_types::PaneId, abs_line: usize) {
        self.focus_req = Some(pane);
        if let Some(view) = self.orch.panes.read().ok().and_then(|m| m.get(&pane).cloned()) {
            if let Ok(mut model) = view.model.lock() {
                model.scroll_to_abs_line(abs_line);
            }
        }
    }
}
