//! AI 명령 바의 "⋯" 더보기 메뉴 — 주제별 하위 메뉴 + 검색.
//!
//! Claude만 해도 명령이 80개가 넘는다(2026-08-21 공식 표 전수 반영). 한 줄로 늘어놓으면
//! 아무도 못 찾으므로 **주제별 묶음**으로 접고, 이름·설명 어느 쪽으로도 **검색**되게 한다.
//! 표기는 바 버튼과 같은 규칙이다 — 한국어 요약명을 보여주고, 실제 슬래시 명령과 설명은
//! 툴팁에 둔다.

use crate::aicmdbar::BarAction;
use crate::aicmdcmds::{secondary_flat, secondary_groups, BarCmd};
use nabi_i18n::{tr, Lang};

/// 검색어를 프레임 사이에 보관할 자리(메뉴는 매 프레임 다시 그려진다).
const FILTER_ID: &str = "ai_more_filter";

/// 메뉴 한 줄. 클릭하면 보낼 동작을 돌려준다.
fn row(ui: &mut egui::Ui, lang: Lang, bc: &BarCmd) -> Option<BarAction> {
    let text = if bc.label.is_empty() { bc.cmd.to_owned() } else { tr(lang, bc.label).to_string() };
    let tip = format!("{} {}", bc.cmd, tr(lang, bc.desc));
    if ui.button(text).on_hover_text(tip).clicked() {
        ui.close();
        return Some(BarAction::Cmd(bc.cmd.to_owned(), bc.opens_ui));
    }
    None
}

/// 검색어와 맞는가 — 명령 문자열·한국어 설명·요약 라벨 어느 쪽이든.
fn hits(bc: &BarCmd, lang: Lang, q: &str) -> bool {
    let cmd = bc.cmd.to_lowercase();
    cmd.contains(q)
        || tr(lang, bc.desc).to_lowercase().contains(q)
        || (!bc.label.is_empty() && tr(lang, bc.label).to_lowercase().contains(q))
}

/// "⋯" 버튼과 그 안의 메뉴를 그린다.
pub(crate) fn more_menu(ui: &mut egui::Ui, lang: Lang, kind: &str, text: egui::RichText) -> Option<BarAction> {
    let mut send = None;
    ui.menu_button(text, |ui| {
        ui.set_min_width(230.0);
        let id = egui::Id::new(FILTER_ID);
        let mut q: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
        let edit = egui::TextEdit::singleline(&mut q)
            .hint_text(tr(lang, "aicb.filter"))
            .desired_width(215.0);
        if ui.add(edit).changed() {
            ui.data_mut(|d| d.insert_temp(id, q.clone()));
        }
        ui.separator();
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            send = by_group(ui, lang, kind);
        } else {
            send = by_search(ui, lang, kind, &q);
        }
    });
    send
}

/// 검색어가 없을 때 — 주제별 하위 메뉴(라벨이 빈 묶음은 그대로 펼친다).
fn by_group(ui: &mut egui::Ui, lang: Lang, kind: &str) -> Option<BarAction> {
    let mut send = None;
    for g in secondary_groups(kind) {
        if g.label.is_empty() {
            for bc in g.cmds {
                if let Some(a) = row(ui, lang, bc) { send = Some(a); }
            }
            continue;
        }
        // 삼각형을 직접 붙이지 않는다 — `menu_button`이 하위 메뉴 화살표를 스스로 그려서
        // 두 개가 나란히 찍혔다(사용자 보고 2026-08-25).
        ui.menu_button(tr(lang, g.label), |ui| {
            ui.set_min_width(200.0);
            for bc in g.cmds {
                if let Some(a) = row(ui, lang, bc) { send = Some(a); }
            }
        });
    }
    send
}

/// 검색어가 있을 때 — 묶음을 무시하고 평평하게, 스크롤로 본다.
fn by_search(ui: &mut egui::Ui, lang: Lang, kind: &str, q: &str) -> Option<BarAction> {
    let mut send = None;
    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        let mut n = 0usize;
        for bc in secondary_flat(kind) {
            if hits(bc, lang, q) {
                n += 1;
                if let Some(a) = row(ui, lang, bc) { send = Some(a); }
            }
        }
        if n == 0 {
            ui.weak(tr(lang, "aicb.nomatch"));
        }
    });
    send
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_command_and_description() {
        let claude = crate::aicmdclaude::groups();
        let all: Vec<_> = claude.iter().flat_map(|g| g.cmds.iter()).collect();
        let color = all.iter().find(|b| b.cmd == "/color").expect("/color 가 목록에 있어야 한다");
        assert!(hits(color, Lang::Ko, "color"));
        assert!(hits(color, Lang::Ko, "/col"));
        assert!(!hits(color, Lang::Ko, "zzzz"));
        // 한국어 설명으로도 찾을 수 있어야 한다(영문 명령을 모르는 사용자).
        let rename = all.iter().find(|b| b.cmd == "/rename").expect("/rename 가 목록에 있어야 한다");
        assert!(hits(rename, Lang::Ko, "이름"), "설명으로 검색되어야 한다");
    }
}
