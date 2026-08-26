//! **명령 블록 목록 창** — 지나간 명령을 한자리에 모아 보고, 눌러서 그 자리로 간다.
//!
//! 모으는 일과 옮기는 일은 `nabi_vt::blocklist`가 한다. 여기는 화면만 그린다.
//!
//! 실패한 것만 보는 거르개를 함께 둔다 — 목록을 여는 까닭은 대개 "무엇이 깨졌나"이고,
//! 이미 있는 `실패로 이동`과 같은 기준(종료 코드가 0이 아님)을 쓴다.

use crate::app::NabiApp;
use nabi_i18n::tr;

impl NabiApp {
    /// 창을 켜고 끈다(팔레트·메뉴에서 부른다).
    pub(crate) fn toggle_block_list(&mut self) {
        self.block_list_open = !self.block_list_open;
    }

    /// 목록 창. 화면 밖에서 값을 모으고, 고른 뒤에 옮긴다(빌림이 겹치지 않게).
    pub(crate) fn show_block_list(&mut self, ctx: &egui::Context) {
        if !self.block_list_open {
            return;
        }
        let lang = self.lang;
        let Some(pane) = self.focused_pane() else {
            self.block_list_open = false;
            return;
        };
        let view = self.orch.panes.read().ok().and_then(|m| m.get(&pane).cloned());
        let blocks = match view.as_ref().and_then(|v| v.model.lock().ok()) {
            Some(m) => m.command_blocks(),
            None => Vec::new(),
        };
        let failed = blocks.iter().filter(|b| b.exit.is_some_and(|c| c != 0)).count();
        let mut open = true;
        let mut goto = None;
        let mut only_failed = self.block_list_failed_only;
        egui::Window::new(tr(lang, "blocks.title"))
            .open(&mut open)
            .default_size([560.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {}", tr(lang, "blocks.count"), blocks.len()));
                    ui.separator();
                    ui.checkbox(&mut only_failed, format!("{} ({failed})", tr(lang, "blocks.failedonly")));
                });
                ui.separator();
                if blocks.is_empty() {
                    // 왜 비었는지 말해 준다 — 셸 통합이 없으면 표식 자체가 안 생긴다.
                    ui.weak(tr(lang, "blocks.empty"));
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for b in &blocks {
                        let bad = b.exit.is_some_and(|c| c != 0);
                        if only_failed && !bad {
                            continue;
                        }
                        let (mark, col) = match b.exit {
                            Some(0) => ("\u{2713}", crate::theme_ui::OK),
                            Some(_) => ("\u{2717}", crate::theme_ui::ERR),
                            None => ("\u{22ef}", ui.visuals().weak_text_color()),
                        };
                        let r = ui.horizontal(|ui| {
                            ui.colored_label(col, mark);
                            let txt = egui::RichText::new(&b.text).monospace();
                            ui.add(egui::Label::new(txt).truncate().selectable(false));
                        });
                        let hit = r.response.interact(egui::Sense::click());
                        let tip = match b.exit {
                            Some(c) if c != 0 => format!("exit {c} \u{b7} {} \u{c904}", b.out_lines),
                            _ => format!("{} \u{c904}", b.out_lines),
                        };
                        if hit.on_hover_text(tip).clicked() {
                            goto = Some(b.abs);
                        }
                    }
                });
            });
        self.block_list_failed_only = only_failed;
        if let Some(abs) = goto {
            if let Some(mut m) = view.as_ref().and_then(|v| v.model.lock().ok()) {
                m.scroll_to_prompt(abs);
            }
        }
        if !open {
            self.block_list_open = false;
        }
    }
}
