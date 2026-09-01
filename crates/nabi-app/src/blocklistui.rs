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
        // 출력을 꺼내는 두 갈래(복사 · 편집기로) — 창을 그리는 동안 모델을 다시 잠그지
        // 않으려고 자리만 받아 두고 아래에서 처리한다.
        let (mut copy_out, mut open_out) = (None, None);
        let mut only_failed = self.block_list_failed_only;
        let mut filter = self.block_list_filter.clone();
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
                // 명령이 쌓이면 목록도 길어진다 — 이름으로 좁힌다(대소문자 무시).
                ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .hint_text(tr(lang, "browser.filter"))
                        .desired_width(f32::INFINITY),
                );
                ui.separator();
                if blocks.is_empty() {
                    // 왜 비었는지 말해 준다 — 셸 통합이 없으면 표식 자체가 안 생긴다.
                    ui.weak(tr(lang, "blocks.empty"));
                    return;
                }
                let flow = filter.to_lowercase();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for b in &blocks {
                        let bad = b.exit.is_some_and(|c| c != 0);
                        if only_failed && !bad {
                            continue;
                        }
                        // 거르는 글자가 있으면 명령 글자에서 찾는다(빈 칸이면 전부 보인다).
                        if !flow.is_empty() && !b.text.to_lowercase().contains(&flow) {
                            continue;
                        }
                        let (mark, col) = match b.exit {
                            Some(0) => ("\u{2713}", crate::theme_ui::OK),
                            Some(_) => ("\u{2717}", crate::theme_ui::ERR),
                            None => ("\u{22ef}", ui.visuals().weak_text_color()),
                        };
                        let r = ui.horizontal(|ui| {
                            ui.colored_label(col, mark);
                            // 걸린 시간은 앞쪽 고정 폭에 — 뒤에 두면 명령 길이에 따라 들쭉날쭉해
                            // 세로로 훑을 수가 없다. 못 잰 것은 비워 둔다(0으로 꾸미지 않는다).
                            let dur = b.ms.map(nabi_vt::human_ms).unwrap_or_default();
                            ui.add_sized(
                                [56.0, ui.spacing().interact_size.y],
                                egui::Label::new(egui::RichText::new(dur).monospace().weak()),
                            );
                            let txt = egui::RichText::new(&b.text).monospace();
                            ui.add(egui::Label::new(txt).truncate().selectable(false));
                        });
                        let hit = r.response.interact(egui::Sense::click());
                        let tip = match b.exit {
                            Some(c) if c != 0 => format!("exit {c} \u{b7} {} \u{c904}", b.out_lines),
                            _ => format!("{} \u{c904}", b.out_lines),
                        };
                        if hit.clone().on_hover_text(tip).clicked() {
                            goto = Some(b.abs);
                        }
                        // 오른쪽 클릭 — 그 블록의 출력만 꺼낸다(마지막 것만 되던 일).
                        hit.context_menu(|ui| {
                            if ui.button(tr(lang, "findall.copy")).clicked() {
                                copy_out = Some(b.abs);
                                ui.close();
                            }
                            if ui.button(tr(lang, "blocks.openout")).clicked() {
                                open_out = Some((b.abs, b.text.clone()));
                                ui.close();
                            }
                        });
                    }
                });
            });
        self.block_list_failed_only = only_failed;
        self.block_list_filter = filter;
        // 상한: 한 블록이 수십만 줄일 수 있다. 잘렸다는 사실은 문서 제목이 아니라 양으로 드러난다.
        let grab = |abs: i64| -> String {
            view.as_ref()
                .and_then(|v| v.model.lock().ok())
                .map(|m| m.block_output(abs, 200_000))
                .unwrap_or_default()
        };
        if let Some(abs) = copy_out {
            let t = grab(abs);
            if !t.is_empty() {
                ctx.copy_text(t);
            }
        }
        if let Some((abs, title)) = open_out {
            let t = grab(abs);
            if !t.is_empty() {
                self.open_text_as_doc(&title, t);
            }
        }
        if let Some(abs) = goto {
            // 그 블록이 스크롤백에서 밀려났으면 못 간다. 조용히 넘기면 목록에는 있는데
            // 눌러도 아무 일이 없어 고장으로 읽힌다(2026-09-01 수정).
            let went = view
                .as_ref()
                .and_then(|v| v.model.lock().ok().map(|mut m| m.scroll_to_prompt(abs)))
                .unwrap_or(false);
            if !went {
                self.notify = Some((
                    nabi_i18n::tr(self.lang, "blocks.gone").to_string(),
                    std::time::Instant::now(),
                ));
            }
        }
        if !open {
            self.block_list_open = false;
        }
    }
}
