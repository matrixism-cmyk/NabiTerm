//! 탭 오른쪽 메뉴의 **출력 보내기** 묶음 — 이 pane 의 출력을 어디론가 보내는 일만 모았다.
//!
//! ## 왜 묶었나
//!
//! 탭 오른쪽 메뉴가 평평한 항목 열아홉 개였다. 그중 여섯이 "출력을 파일로 · 클립보드로 ·
//! AI 에게 · 마크다운으로" 처럼 **같은 질문의 답들**이라, 나머지 열셋 사이에 흩어져 있으면
//! 찾는 사람은 열아홉 개를 다 읽어야 한다.
//!
//! 하나로 묶으면 위 칸은 열넷이 되고, 출력을 다루려는 사람은 그 하나만 열면 된다.
//! 이미 있던 "추출" 서브메뉴(URL·IP·메일·숫자)도 여기 안으로 들어온다 — 그것도 결국
//! 출력을 클립보드로 보내는 일이다.

use nabi_i18n::{tr, Lang};
use nabi_orchestrator::OrchestratorHandle;
use nabi_types::PaneId;

/// 출력 보내기 서브메뉴를 그린다.
///
/// `ai_handoff` 는 (pane, 마크다운으로?) 를 담고, `save_msg` 는 저장 결과를 알릴 자리다.
pub(crate) fn output_menu(
    ui: &mut egui::Ui,
    tab: &PaneId,
    orch: &OrchestratorHandle,
    lang: Lang,
    ai_handoff: &mut Option<(PaneId, bool)>,
    save_msg: &mut Option<String>,
) {
    ui.menu_button(tr(lang, "term.outputmenu"), |ui| {
        // 화면+스크롤백을 글로 펼친다. 여기 있는 모든 항목이 이것 하나를 나눠 쓴다.
        let dump = || orch.panes.read().ok().and_then(|m| m.get(tab).cloned()).and_then(|v| v.model.lock().ok().map(|md| md.dump_text(1_000_000)));
        // 터미널→AI 동선(팔레트와 동일 기능 — 표면 정합): 마지막 명령을 AI에/클립보드로.
        if ui.button(tr(lang, "handoff.last")).clicked() {
            *ai_handoff = Some((*tab, false));
            ui.close();
        }
        if ui.button(tr(lang, "handoff.copymd")).clicked() {
            *ai_handoff = Some((*tab, true));
            ui.close();
        }
        ui.separator();
        if ui.button(tr(lang, "menu.saveoutput")).clicked() {
            // 저장은 대개 **남겨 두려고** 한다. 실패했는데 아무 말이 없으면 파일이 생긴 줄 알고,
            // 필요할 때가 되어서야 없다는 것을 안다(내보내기에서 겪은 것과 같은 결함).
            if let Some(text) = dump() {
                if let Some(path) = rfd::FileDialog::new().set_file_name("terminal-output.txt").save_file() {
                    *save_msg = Some(match std::fs::write(&path, text) {
                        Ok(()) => format!("\u{2713} {}", path.display()),
                        Err(e) => format!("\u{2715} {}: {e}", path.display()),
                    });
                }
            }
            ui.close();
        }
        // 출력을 클립보드로 복사(파일 저장 없이) + AI용 마크다운 코드블록 복사(바이브코딩).
        if ui.button(tr(lang, "term.copyoutput")).clicked() {
            if let Some(t) = dump() { ui.ctx().copy_text(t); }
            ui.close();
        }
        if ui.button(tr(lang, "term.copyoutputmd")).clicked() {
            if let Some(t) = dump() { ui.ctx().copy_text(format!("```\n{}\n```", t.trim_end())); }
            ui.close();
        }
        // 출력에서 URL/IP/이메일/숫자만 추출해 클립보드로(T3-1; 에디터 추출 메뉴와 같은 라벨).
        ui.menu_button(tr(lang, "editor.extractmenu"), |ui| {
            type Ext = fn(&str) -> String;
            for (key, f) in [
                ("term.exturls", crate::editorextract::extract_urls as Ext),
                ("term.extips", crate::editorextract::extract_ips as Ext),
                ("term.extemails", crate::editorextract::extract_emails as Ext),
                ("term.extnumbers", crate::editorextract::extract_numbers as Ext),
            ] {
                if ui.button(tr(lang, key)).clicked() {
                    if let Some(t) = dump() { ui.ctx().copy_text(f(&t)); }
                    ui.close();
                }
            }
        });
    });
}
