//! 브로드캐스트(일괄 명령) 결과 집계 뷰(T7-3) — 대상 pane별 상태를 한 표로.
//!
//! 한국 사용자 선택 기준 상위 기능(다중 서버 일괄 명령)의 마무리: 명령을 뿌린 뒤
//! "어디는 성공했고 어디서 실패했나"를 탭을 돌지 않고 확인한다. 셸 통합(OSC 133)의
//! pane별 종료코드·실행시간과 화면 마지막 줄을 실시간으로 보여 준다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;

impl NabiApp {
    pub(crate) fn show_broadcast_results(&mut self, ctx: &egui::Context) {
        if !self.bcast_view_open {
            return;
        }
        let lang = self.lang;
        // 대상 = 그룹이 있으면 그룹, 없으면 현재 도크의 터미널 전체(브로드캐스트 규칙과 동일).
        let targets: Vec<PaneId> = if self.broadcast_group.is_empty() {
            self.dock
                .iter_all_tabs()
                .map(|(_, p)| *p)
                .filter(|p| {
                    !self.editors.contains_key(p)
                        && !self.browser_tabs.contains_key(p)
                        && Some(*p) != self.sftp_pane
                        && !self.sftp_bg.contains_key(p)
                })
                .collect()
        } else {
            let mut v: Vec<PaneId> = self.broadcast_group.iter().copied().collect();
            v.sort();
            v
        };
        // 실패를 맨 위로(무엇이 잘못됐는지부터) — 실패(0) > 실행중(1) > 미상(2) > 성공(3).
        let mut targets = targets;
        targets.sort_by_key(|p| match (self.cmd_start.contains_key(p), self.last_exit.get(p)) {
            (false, Some(c)) if *c != 0 => 0u8,
            (true, _) => 1,
            (false, None) => 2,
            (false, Some(_)) => 3,
        });
        let mut open = true;
        egui::Window::new(tr(lang, "bcast.results"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([560.0, 320.0])
            .show(ctx, |ui| {
                if !self.broadcast {
                    ui.weak(tr(lang, "bcast.off"));
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("bcast_grid").num_columns(4).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                        ui.strong(tr(lang, "bcast.col.pane"));
                        ui.strong(tr(lang, "bcast.col.state"));
                        ui.strong(tr(lang, "bcast.col.dur"));
                        ui.strong(tr(lang, "bcast.col.lastline"));
                        ui.end_row();
                        let mut focus = None;
                        for p in &targets {
                            let title = self
                                .orch
                                .panes
                                .read()
                                .ok()
                                .and_then(|m| m.get(p).map(|v| v.title.clone()))
                                .unwrap_or_default();
                            // 클릭 → 해당 pane 탭 활성화(실패 원인 확인 동선 단축).
                            if ui.selectable_label(false, format!("#{} {title}", p.get())).on_hover_text(tr(lang, "bcast.focus")).clicked() {
                                focus = Some(*p);
                            }
                            // 상태: 실행 중(⚙) > 종료코드(✓/✗ N) > 미상.
                            if self.cmd_start.contains_key(p) {
                                ui.colored_label(crate::theme_ui::BROADCAST, "\u{2699}");
                            } else {
                                match self.last_exit.get(p) {
                                    Some(0) => { ui.colored_label(crate::theme_ui::OK, "\u{2713} 0"); }
                                    Some(c) => { ui.colored_label(crate::theme_ui::ERR, format!("\u{2717} {c}")); }
                                    None => { ui.weak("\u{2014}"); }
                                }
                            }
                            match self.last_duration.get(p) {
                                Some(ms) => ui.label(crate::statusfmt::human_duration(*ms)),
                                None => ui.weak("\u{2014}"),
                            };
                            // 화면 마지막 비어있지 않은 줄(잘라서) — 실패 사유 한눈에.
                            let last = self
                                .orch
                                .panes
                                .read()
                                .ok()
                                .and_then(|m| m.get(p).map(|v| v.model.clone()))
                                .and_then(|md| md.lock().ok().map(|m| m.visible_bottom_text(3)))
                                .unwrap_or_default();
                            let line = last.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                            let short: String = line.chars().take(60).collect();
                            ui.monospace(short);
                            ui.end_row();
                        }
                        if let Some(p) = focus {
                            if let Some(loc) = self.dock.find_tab(&p) {
                                let _ = self.dock.set_active_tab(loc);
                            }
                        }
                    });
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(500)); // 라이브 갱신.
            });
        if !open {
            self.bcast_view_open = false;
        }
    }
}
