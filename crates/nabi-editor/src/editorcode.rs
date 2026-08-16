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
