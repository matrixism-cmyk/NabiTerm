//! 설정 ▸ 편집기의 **언어 서버** 묶음 — 배치 V의 남은 phase(M1).
//!
//! `lspservers::SERVERS`가 아는 언어를 늘어놓고, 각각이 **이 PC에 깔려 있는지** 보여 준다.
//!
//! ## 왜 목록을 손으로 적지 않는가
//!
//! 서버를 하나 더하면 설정 화면도 같이 고쳐야 한다면 언젠가 잊는다. 그래서 이 화면은
//! `SERVERS` 표를 그대로 읽는다 — **표가 유일한 출처**이고, 화면은 그것을 비출 뿐이다.
//!
//! ## 왜 상태를 보여 주는가
//!
//! LSP는 **깔려 있지 않은 것이 기본**이라 아무 일도 일어나지 않는 것이 정상이다. 그런데
//! 사용자 입장에서 "정의로 가기"가 안 되는 것과 고장 난 것은 구별되지 않는다. 무엇이
//! 없어서 안 되는지 이름으로 보여 주면 스스로 고칠 수 있다.

use nabi_i18n::{tr, Lang};

/// 편집기 설정 아래에 붙는 언어 서버 묶음.
///
/// 자유 함수인 이유: nabiPad는 **자체 설정 창**을 가지고 있고 그 창은 앱 상태를 모른다.
/// 메서드로 두면 같은 화면을 두 벌 만들게 된다.
pub(crate) fn lsp_group(ui: &mut egui::Ui, lang: Lang) {
    ui.add_space(10.0);
    ui.separator();
    ui.label(egui::RichText::new(tr(lang, "settings.lsp")).strong());
    ui.add(egui::Label::new(
        egui::RichText::new(tr(lang, "settings.lsp.hint")).weak().small(),
    ).wrap());
    ui.add_space(6.0);

    for s in nabi_editor::lspservers::SERVERS {
        let found = nabi_pty::resolve_program(s.cmd);
        ui.horizontal(|ui| {
            // 있으면 초록 체크, 없으면 조용한 회색 — 없는 것이 정상이라 붉게 칠하지 않는다.
            match found.is_some() {
                true => { ui.colored_label(crate::theme_ui::OK, "\u{2713}"); }
                false => { ui.weak("\u{2014}"); }
            }
            ui.label(s.label);
            ui.add_space(4.0);
            let cmd = egui::RichText::new(s.cmd).monospace().small();
            match &found {
                Some(p) => {
                    ui.weak(cmd).on_hover_text(p.to_string_lossy());
                }
                None => {
                    ui.weak(cmd).on_hover_text(tr(lang, "settings.lsp.missing"));
                }
            }
        });
    }
    ui.add_space(4.0);
    ui.add(egui::Label::new(
        egui::RichText::new(tr(lang, "settings.lsp.install")).weak().small(),
    ).wrap());
}
