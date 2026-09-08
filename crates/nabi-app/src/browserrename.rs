//! 로컬 탐색기의 **일괄 이름변경** 창(배치 AJ).
//!
//! 규칙은 [`crate::renamerule`] 한 곳에 있다 — 원격(SFTP)과 같은 규칙을 쓴다.
//! 여기는 고를 것을 받고 계획을 보여 주고 실행만 한다.
//!
//! ## ⚠️ 원격에는 이것을 쓰지 않는다 — 거기엔 이미 자기 것이 있다
//!
//! 2026-09-07에 "원격에는 일괄 이름 바꾸기가 없다"고 보고 이 창을 원격까지 넓혔다가
//! **되돌렸다.** 원격에는 처음부터 있었다 — 툴바 도구 메뉴가 켜는 **인라인 막대**이고
//! (`sftptab.rs`의 `batch_mode`), `{n}`/`{nn}`/`{name}`/`{ext}` 토큰까지 있으며,
//! 이름 충돌 검사도 같은 `renamerule::plan_batch` 로 이미 하고 있다.
//!
//! 내가 `browser.batchrename` 이라는 **키 이름으로** grep 해서 없다고 판단한 것이다.
//! 원격의 키는 `sftp.batchrename` 이었다. 이름이 다른 짝은 규칙이지 예외가 아니다.
//!
//! 진짜 구멍은 **가는 길**이었다: 로컬은 우클릭으로 닿는데 원격은 툴바를 거쳐야 했다.
//! 그래서 원격 우클릭 메뉴는 있는 막대를 켜기만 한다(`sftplist.rs`).
//!
//! ## 왜 미리 보여 주는가
//!
//! 이름 바꾸기는 **되돌리기가 없다.** 스무 개를 한꺼번에 바꾸고 나서 잘못됐다는 것을
//! 알면 손으로 스무 번 되돌려야 하고, 그때는 원래 이름을 기억해야 한다.
//!
//! 그래서 실행 전에 `이전 → 이후` 를 그대로 보여 준다. 계획이 통째로 거절되면(이름 충돌)
//! **왜 거절됐는지**도 함께 보여 준다 — "안 됩니다"만으로는 무엇을 고쳐야 할지 모른다.

use crate::app::NabiApp;
use nabi_i18n::tr;

/// 창 상태. 닫으면 통째로 버린다.
#[derive(Default, Clone)]
pub(crate) struct BatchRename {
    pub find: String,
    pub replace: String,
    /// 창을 열 때 고정한 대상 — 여는 동안 선택이 바뀌어도 계획이 흔들리지 않게.
    pub names: Vec<String>,
}

impl NabiApp {
    /// 선택한 파일들로 창을 연다. 아무것도 안 골랐으면 알리고 열지 않는다.
    pub(crate) fn open_batch_rename(&mut self) {
        let mut names: Vec<String> = self.browser.multi.iter().cloned().collect();
        if names.is_empty() {
            // 조용히 아무 일도 안 하면 사용자는 메뉴가 고장 난 줄 안다.
            self.notify = Some((tr(self.lang, "browser.batchrename.none").to_string(), std::time::Instant::now()));
            return;
        }
        names.sort();
        self.batch_rename = Some(BatchRename { names, ..Default::default() });
    }

    /// 창을 그린다.
    pub(crate) fn show_batch_rename(&mut self, ctx: &egui::Context) {
        let Some(st) = self.batch_rename.clone() else { return };
        let lang = self.lang;
        let mut next = st.clone();
        let (mut close, mut apply) = (false, false);
        crate::modal::foreground_modal(ctx, "batch_rename", |ui| {
            ui.heading(tr(lang, "browser.batchrename"));
            ui.weak(format!("{}: {}", tr(lang, "menu.selectall"), st.names.len()));
            ui.add_space(4.0);
            egui::Grid::new("br_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(tr(lang, "replace.find"));
                ui.add(egui::TextEdit::singleline(&mut next.find).desired_width(240.0));
                ui.end_row();
                ui.label(tr(lang, "replace.to"));
                ui.add(egui::TextEdit::singleline(&mut next.replace).desired_width(240.0));
                ui.end_row();
            });
            ui.add_space(4.0);
            let plan = crate::renamerule::plan_batch(&st.names, &next.find, &next.replace, lang);
            match &plan {
                Err(e) => {
                    // 왜 거절됐는지 말한다 — "안 됩니다"만으로는 무엇을 고칠지 모른다.
                    ui.colored_label(crate::theme_ui::ERR, e);
                }
                Ok(p) if p.is_empty() => {
                    ui.weak(tr(lang, "browser.batchrename.nomatch"));
                }
                Ok(p) => {
                    ui.label(format!("{}: {}", tr(lang, "find.willchange"), p.len()));
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (from, to) in p.iter().take(50) {
                            ui.horizontal(|ui| {
                                ui.monospace(from);
                                ui.weak("\u{2192}");
                                ui.monospace(to);
                            });
                        }
                    });
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = plan.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
                if ui.add_enabled(ok, egui::Button::new(tr(lang, "sftp.rename"))).clicked() {
                    apply = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    close = true;
                }
            });
        });
        if apply {
            self.run_batch_rename(&st.names, &next.find, &next.replace);
            close = true;
        }
        self.batch_rename = if close { None } else { Some(next) };
    }

    /// 계획대로 바꾼다. **실패한 것은 세지 않고 이름을 알린다.**
    fn run_batch_rename(&mut self, names: &[String], find: &str, replace: &str) {
        let Ok(plan) = crate::renamerule::plan_batch(names, find, replace, self.lang) else { return };
        let dir = self.browser.path.clone();
        let (mut done, mut failed) = (0usize, Vec::new());
        for (from, to) in plan {
            match std::fs::rename(dir.join(&from), dir.join(&to)) {
                Ok(()) => done += 1,
                Err(e) => failed.push(format!("{from}: {e}")),
            }
        }
        let msg = match failed.is_empty() {
            true => format!("{} {done}", tr(self.lang, "browser.batchrename")),
            // 못 바꾼 것을 세지 않는다 — 오늘 배치 AF 에서 고친 것과 같은 결함이다.
            false => format!("{} {} \u{00b7} {done}", tr(self.lang, "replace.unwritable"), failed.join(", ")),
        };
        self.notify = Some((msg, std::time::Instant::now()));
        // 목록은 매 프레임 `path` 에서 다시 읽으므로 따로 새로고침할 것이 없다.
        // 다만 **선택은 비운다** — 이름이 바뀌어 옛 이름을 고르고 있으면 다음 동작이
        // 없는 파일을 가리킨다.
        self.browser.multi.clear();
        self.browser.selected = None;
    }
}
