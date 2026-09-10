//! 명령 기록 창 — 무엇을 실행했고 무엇이 실패했는지 되찾는 곳.
//!
//! 기록은 진작부터 모이고 있었는데(`cmdhist`) 볼 방법이 팔레트 몇 줄뿐이었다. 모아 놓고
//! 되찾을 수 없으면 모으지 않은 것과 같다.
//!
//! 고르는 규칙은 전부 `cmdhistfilter`의 순수 함수다 — 여기서는 그리기만 한다.

use crate::app::NabiApp;
use crate::cmdhistfilter::{select, short, Filter};
use nabi_i18n::tr;

/// 한 번에 보여 줄 최대 줄. 넘으면 **몇 개가 잘렸는지 함께 말한다.**
const LIMIT: usize = 200;

impl NabiApp {
    /// 도구 메뉴·팔레트에서 연다.
    pub(crate) fn open_cmd_history(&mut self) {
        self.cmd_hist_open = true;
    }

    /// 열려 있으면 그린다.
    pub(crate) fn show_cmd_history(&mut self, ctx: &egui::Context) {
        if !self.cmd_hist_open {
            return;
        }
        let lang = self.lang;
        let mut open = true;
        let cwd = self
            .focused_pane()
            .and_then(|p| self.cwds.get(&p))
            .map(|c| crate::workspace::strip_uri_slash(c))
            .unwrap_or_default();
        let q_id = egui::Id::new("cmdhist_q");
        let f_id = egui::Id::new("cmdhist_f");
        let mut query: String = ctx.data(|d| d.get_temp(q_id)).unwrap_or_default();
        let mut filter: Filter = ctx.data(|d| d.get_temp(f_id)).unwrap_or_default();
        let mut run: Option<String> = None;
        let mut copy: Option<String> = None;
        // 실행하지 않고 프롬프트에 **올려만 두기**, 그리고 그때 있던 폴더로 가기.
        let mut put: Option<String> = None;
        let mut go_cwd: Option<String> = None;
        let redacting = self.config.terminal.redact_history;
        egui::Window::new(tr(lang, "cmdhist.title"))
            .open(&mut open)
            .default_size([720.0, 480.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("\u{1f50d}");
                    ui.add(
                        egui::TextEdit::singleline(&mut query)
                            .hint_text(tr(lang, "cmdhist.search"))
                            .desired_width(240.0),
                    );
                    ui.checkbox(&mut filter.failed_only, tr(lang, "cmdhist.failed"));
                    ui.add_enabled(!cwd.is_empty(), egui::Checkbox::new(&mut filter.this_dir_only, tr(lang, "cmdhist.thisdir")))
                        .on_disabled_hover_text(tr(lang, "cmdhist.nodir"));
                    // 가려진 값이 보이는 까닭을 알려 준다 — 모르면 기록이 깨진 줄 안다.
                    if redacting {
                        ui.separator();
                        ui.weak(tr(lang, "cmdhist.redacted")).on_hover_text(tr(lang, "settings.redacthisthint"));
                    }
                });
                ui.separator();
                let (rows, cut) = select(&self.config.terminal.cmd_history, &query, filter, &cwd, LIMIT);
                if rows.is_empty() {
                    ui.weak(tr(lang, "cmdhist.none"));
                    return;
                }
                egui::ScrollArea::vertical().id_salt("cmdhist_rows").auto_shrink([false, false]).show(ui, |ui| {
                    for r in &rows {
                        ui.horizontal(|ui| {
                            mark(ui, r.exit);
                            let row = ui.selectable_label(false, short(&r.cmd, 70)).on_hover_text(&r.cmd);
                            if row.clicked() {
                                run = Some(r.cmd.clone());
                            }
                            // 우클릭 — 누르면 곧바로 실행되므로, **실행하지 않는 길**이
                            // 있어야 한다(기록에는 되돌릴 수 없는 명령도 들어 있다).
                            row.context_menu(|ui| {
                                if ui.button(tr(lang, "cmdhist.run")).clicked() {
                                    run = Some(r.cmd.clone());
                                    ui.close();
                                }
                                if ui.button(tr(lang, "cmdhist.put")).clicked() {
                                    put = Some(r.cmd.clone());
                                    ui.close();
                                }
                                if ui.button(tr(lang, "menu.copy")).clicked() {
                                    copy = Some(r.cmd.clone());
                                    ui.close();
                                }
                                if !r.cwd.is_empty() && ui.button(tr(lang, "cmdhist.gocwd")).clicked() {
                                    go_cwd = Some(r.cwd.clone());
                                    ui.close();
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("\u{1f4cb}").on_hover_text(tr(lang, "menu.copy")).clicked() {
                                    copy = Some(r.cmd.clone());
                                }
                                ui.weak(short(&r.cwd, 30));
                                // 얼마나 걸렸는지. 옛 기록에는 없으므로 있을 때만 적는다.
                                if let Some(sec) = r.secs(&self.config.terminal.cmd_secs) {
                                    ui.weak(crate::cmdhist::human_secs(sec));
                                }
                            });
                        });
                    }
                });
                // 조용히 자르지 않는다 — 잘렸으면 몇 개인지 말한다.
                if cut > 0 {
                    ui.weak(format!("{} +{cut}", tr(lang, "cmdhist.more")));
                }
            });
        ctx.data_mut(|d| {
            d.insert_temp(q_id, query);
            d.insert_temp(f_id, filter);
        });
        if let Some(c) = copy {
            ctx.copy_text(c);
        }
        if let Some(c) = run {
            // 팔레트의 재실행과 **같은 길**을 쓴다 — 두 벌로 나뉘면 곧 어긋난다.
            self.run_history_cmd(c);
            self.cmd_hist_open = false;
        }
        if let Some(c) = put {
            // 올려만 두고 창을 닫는다 — 고칠 자리가 프롬프트이므로 거기로 보내야 한다.
            self.put_history_cmd(c, false);
            self.cmd_hist_open = false;
        }
        if let Some(p) = go_cwd {
            // 그때 그 폴더로 — 없어졌으면 아무 일도 하지 않는다(빈 화면으로 데려가지 않는다).
            let dir = std::path::PathBuf::from(p);
            if dir.is_dir() {
                self.browser.path = dir;
                self.browser.open = true;
                self.browser.cache_dirty = true;
            } else {
                self.notify = Some((tr(lang, "cmdhist.gone").to_string(), std::time::Instant::now()));
            }
        }
        if !open {
            self.cmd_hist_open = false;
        }
    }
}

/// 성공·실패 표시. 종료 코드를 숫자로만 보여 주면 눈에 안 들어온다.
fn mark(ui: &mut egui::Ui, exit: i32) {
    let (glyph, color) = match exit {
        0 => ("\u{2713}", egui::Color32::from_rgb(0x3c, 0xa8, 0x55)),
        _ => ("\u{2717}", egui::Color32::from_rgb(0xd0, 0x4a, 0x3a)),
    };
    ui.colored_label(color, glyph).on_hover_text(format!("exit {exit}"));
}
