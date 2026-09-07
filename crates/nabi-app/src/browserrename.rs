//! 탐색기·SFTP 공용 **일괄 이름변경** 창(배치 AJ, 2026-09-07에 원격까지 넓힘).
//!
//! 규칙은 [`crate::renamerule`] 한 곳에 있다. 여기는 고를 것을 받고 계획을 보여 주고
//! 실행만 한다.
//!
//! ## 왜 원격에도 같은 창을 쓰는가
//!
//! 2026-09-07 쌍둥이 비교에서 이 창이 **로컬 탐색기에만** 붙어 있는 것을 봤다. 그런데
//! 규칙(`renamerule::plan_batch`)은 처음부터 공용이었고, 거기에 **이름 충돌 검사**가 들어
//! 있다. 원격에는 그 창이 없으니 검사도 함께 없었던 셈이다 — `posix-rename` 을 지원하는
//! 서버에서는 덮어쓰기가 조용히 성공한다.
//!
//! 그래서 창을 새로 만들지 않고 **대상만 바꿔** 쓴다. 다른 점은 두 가지뿐이다:
//! 이름을 어디서 고르는가(`browser.multi` ↔ `sftp.multi`), 그리고 무엇으로 바꾸는가
//! (`fs::rename` ↔ `Command::SftpRename`).
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
    /// 원격(SFTP) 목록을 바꾸는 중인가. 창을 열 때 고정한다 — 여는 동안 탭을 옮겨도
    /// 엉뚱한 쪽을 바꾸지 않게.
    pub remote: bool,
}

impl NabiApp {
    /// 선택한 파일들로 창을 연다. 아무것도 안 골랐으면 알리고 열지 않는다.
    pub(crate) fn open_batch_rename(&mut self, remote: bool) {
        let src = match remote {
            true => &self.sftp.multi,
            false => &self.browser.multi,
        };
        let mut names: Vec<String> = src.iter().cloned().collect();
        if names.is_empty() {
            // 조용히 아무 일도 안 하면 사용자는 메뉴가 고장 난 줄 안다.
            self.notify = Some((tr(self.lang, "browser.batchrename.none").to_string(), std::time::Instant::now()));
            return;
        }
        names.sort();
        self.batch_rename = Some(BatchRename { names, remote, ..Default::default() });
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
            self.run_batch_rename(&st.names, &next.find, &next.replace, st.remote);
            close = true;
        }
        self.batch_rename = if close { None } else { Some(next) };
    }

    /// 계획대로 바꾼다. **실패한 것은 세지 않고 이름을 알린다.**
    fn run_batch_rename(&mut self, names: &[String], find: &str, replace: &str, remote: bool) {
        let Ok(plan) = crate::renamerule::plan_batch(names, find, replace, self.lang) else { return };
        if remote {
            self.run_batch_rename_remote(plan);
            return;
        }
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

impl NabiApp {
    /// 원격에서 계획대로 바꾼다.
    ///
    /// 로컬과 달리 **여기서는 결과를 알 수 없다.** 이름 바꾸기는 명령으로 보내고 응답은
    /// 나중에 이벤트로 온다. 그래서 "몇 개 성공"이 아니라 "몇 개 보냄"을 알린다 —
    /// 실패는 그때 각자 알린다. 안 온 답을 성공으로 세는 것이 더 나쁘다.
    fn run_batch_rename_remote(&mut self, plan: Vec<(String, String)>) {
        let Some(id) = self.sftp.id else {
            self.notify = Some((
                tr(self.lang, "copyid.needsftp").to_string(),
                std::time::Instant::now(),
            ));
            return;
        };
        let dir = self.sftp.path.clone();
        let n = plan.len();
        for (from, to) in plan {
            self.orch.send(nabi_proto::Command::SftpRename {
                id,
                from: crate::sftppath::join_path(&dir, &from),
                to: crate::sftppath::join_path(&dir, &to),
            });
        }
        self.notify = Some((
            format!("{} {n}", tr(self.lang, "browser.batchrename")),
            std::time::Instant::now(),
        ));
        // 옛 이름을 고르고 있으면 다음 동작이 없는 파일을 가리킨다(로컬과 같은 규칙).
        self.sftp.multi.clear();
        self.sftp.selected = None;
    }
}
