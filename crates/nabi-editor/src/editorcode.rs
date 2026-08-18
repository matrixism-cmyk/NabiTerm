//! LSP 코드 팝업 3종(T6-4 2단계) — 심볼 정보(hover)·참조 목록·진단 목록.
//!
//! 에디터 탭 내부에서 그리므로 분리 창(viewport)에서도 올바른 창에 뜬다.
//! 다른 파일로의 점프는 `EditorAct::open_at`으로 앱에 위임한다.

use crate::editor::{EditorAct, EditorDoc};
use nabi_i18n::{tr, Lang};

const ERR: egui::Color32 = egui::Color32::from_rgb(235, 80, 80);
const AMBER: egui::Color32 = egui::Color32::from_rgb(255, 176, 32);

/// 열려 있는 코드 팝업을 그린다(닫힘/점프는 doc에 반영, 타 파일 열기는 act로).
pub fn show_code_popups(ui: &egui::Ui, doc: &mut EditorDoc, lang: Lang, act: &mut EditorAct) {
    let ctx = ui.ctx().clone();
    // 심볼 정보(hover) — 마크다운 코드펜스는 걷어내고 등폭으로.
    if let Some(info) = doc.lsp_info.clone() {
        let mut open = true;
        egui::Window::new(tr(lang, "lsp.hover")).id(egui::Id::new(("lsp_info", doc.path.clone())))
            .open(&mut open).collapsible(false).default_size([460.0, 220.0]).show(&ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let clean: String = info.lines().filter(|l| !l.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n");
                    ui.monospace(clean);
                });
            });
        if !open { doc.lsp_info = None; }
    }
    // 참조 목록 — 클릭 시 같은 파일이면 즉시 점프, 다른 파일이면 앱에 위임.
    if let Some(refs) = doc.lsp_refs.clone() {
        let mut open = true;
        let mut jump: Option<(String, usize)> = None;
        egui::Window::new(format!("{} ({})", tr(lang, "lsp.refs"), refs.len()))
            .id(egui::Id::new(("lsp_refs", doc.path.clone())))
            .open(&mut open).collapsible(false).default_size([520.0, 260.0]).show(&ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (p, line, col) in &refs {
                        let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                        if ui.selectable_label(false, format!("{name}:{}:{}", line + 1, col + 1)).on_hover_text(p).clicked() {
                            jump = Some((p.clone(), *line as usize));
                        }
                    }
                });
            });
        if let Some((p, line)) = jump {
            if std::path::Path::new(&p) == doc.path { doc.jump_to_line(line); } else { act.open_at = Some((p, line)); }
            open = false;
        }
        if !open { doc.lsp_refs = None; }
    }
    // 자동완성 팝업(T6-4 3단계) — 커서 아래 고정, 앵커 이후 접두어로 필터, 클릭=삽입.
    if let Some(items) = doc.lsp_comp.clone() {
        // 앵커 이후 타이핑분(접두어). 커서가 앵커 앞으로 갔거나 식별자 밖 문자가 끼면 닫는다.
        let prefix: Option<String> = (doc.cur_off >= doc.comp_anchor)
            .then(|| doc.text.chars().skip(doc.comp_anchor).take(doc.cur_off - doc.comp_anchor).collect::<String>())
            .filter(|p| p.chars().all(|c| c.is_alphanumeric() || c == '_'));
        match prefix {
            None => doc.lsp_comp = None,
            Some(pre) => {
                let pl = pre.to_lowercase();
                let vis: Vec<_> = items.iter().filter(|i| i.label.to_lowercase().starts_with(&pl)).take(12).collect();
                if vis.is_empty() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    doc.lsp_comp = None;
                } else {
                    // 키보드 선택: ↑↓ 이동, Enter/Tab 확정 — 팝업이 열린 동안만 에디터에서 가로챈다.
                    let sid = egui::Id::new(("lsp_comp_sel", doc.path.clone()));
                    let mut sel: usize = ctx.data(|d| d.get_temp(sid)).unwrap_or(0);
                    let mut kb_commit = false;
                    ctx.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) { sel += 1; }
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) { sel = sel.saturating_sub(1); }
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) { kb_commit = true; }
                    });
                    sel = sel.min(vis.len() - 1);
                    ctx.data_mut(|d| d.insert_temp(sid, sel));
                    let mut chosen = kb_commit.then(|| vis[sel].insert.clone());
                    egui::Area::new(egui::Id::new(("lsp_comp", doc.path.clone())))
                        .fixed_pos(egui::pos2(doc.cursor_px.0, doc.cursor_px.1 + 2.0))
                        .order(egui::Order::Foreground)
                        .show(&ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_max_width(420.0);
                                for (i, it) in vis.iter().enumerate() {
                                    let row = ui.selectable_label(i == sel, &it.label);
                                    let row = if it.detail.is_empty() { row } else { row.on_hover_text(&it.detail) };
                                    if row.clicked() {
                                        chosen = Some(it.insert.clone());
                                    }
                                }
                            });
                        });
                    if let Some(ins) = chosen {
                        let (text, cur) = crate::lspcomp::commit_completion(&doc.text, doc.comp_anchor, doc.cur_off, &ins);
                        doc.text = text;
                        doc.dirty = true;
                        doc.find.pending_cursor = Some(cur); // 다음 프레임 커서를 삽입 끝으로.
                        doc.lsp_comp = None;
                    }
                }
            }
        }
    }
    // 이름 바꾸기 입력 — 확정 시 act로 앱 허브에 전달(rust-analyzer rename).
    if doc.rename_open {
        let mut open = true;
        let sid = egui::Id::new(("lsp_rename", doc.path.clone()));
        let mut name: String = ctx.data(|d| d.get_temp(sid)).unwrap_or_default();
        egui::Window::new(tr(lang, "lsp.rename.title")).id(sid.with("w"))
            .open(&mut open).collapsible(false).resizable(false).show(&ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr(lang, "lsp.rename.hint")).desired_width(220.0));
                crate::uiutil::focus_once(&r); // 매 프레임 request_focus는 IME 조합 파괴(egui 0.36).
                let go = (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(tr(lang, "common.ok")).clicked();
                if go && !name.trim().is_empty() {
                    act.lsp_rename = Some(name.trim().to_string());
                    name.clear();
                    doc.rename_open = false;
                }
            });
        ctx.data_mut(|d| d.insert_temp(sid, name));
        if !open { doc.rename_open = false; }
    }
    // 진단 목록 — 상태바 오류/경고 클릭으로 열림, 클릭 시 그 줄로.
    if doc.diag_popup {
        let mut open = true;
        let mut jump = None;
        egui::Window::new(format!("{} ({})", tr(lang, "lsp.diags"), doc.diags.len()))
            .id(egui::Id::new(("lsp_diags", doc.path.clone())))
            .open(&mut open).collapsible(false).default_size([560.0, 260.0]).show(&ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (line, sev, msg) in &doc.diags {
                        let (mark, color) = if *sev == 1 { ("\u{2717}", ERR) } else { ("\u{26a0}", AMBER) };
                        ui.horizontal(|ui| {
                            ui.colored_label(color, mark);
                            let short: String = msg.chars().take(120).collect();
                            if ui.selectable_label(false, format!("{}: {short}", line + 1)).on_hover_text(msg).clicked() {
                                jump = Some(*line);
                            }
                        });
                    }
                    if doc.diags.is_empty() { ui.weak(tr(lang, "lsp.nodiags")); }
                });
            });
        if let Some(line) = jump { doc.jump_to_line(line); }
        doc.diag_popup = open;
    }
}
