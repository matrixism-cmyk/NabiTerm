//! 세션 사이드바 다중 선택 + 선택 항목 일괄 연결.
//!
//! 실사용자 피드백(2026-08-21): `.ssh/config`에서 수백 개를 들여오면 목록을 감당할 수 없고,
//! "폴더 전체 연결" 말고 **원하는 것만 골라 한 번에** 접속하고 싶다는 요청.
//!
//! 선택 판정은 순수 함수로 두고(테스트 가능) 렌더링은 sidebar.rs가 맡는다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_session::SavedSession;
use std::collections::HashSet;

/// 행 클릭의 해석 결과.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RowClick {
    /// 평범한 클릭 — 곧바로 연결한다(기존 동작).
    Connect(String),
    /// Ctrl/Shift 클릭 — 선택만 바꾸고 연결하지 않는다.
    Marked,
}

/// 클릭 한 번을 선택 상태에 반영한다.
///
/// - **Ctrl**: 그 항목만 켜고 끈다(앵커도 그 자리로).
/// - **Shift**: 앵커부터 이 항목까지 **보이는 순서 그대로** 범위 선택(앵커가 없으면 Ctrl과 동일).
/// - **그냥 클릭**: 선택을 비우고 연결 — 선택해 놓고 실수로 하나를 눌렀을 때
///   엉뚱한 것이 남아 있지 않게 한다.
pub(crate) fn apply_click(
    marked: &mut HashSet<String>,
    anchor: &mut Option<String>,
    order: &[String],
    name: &str,
    ctrl: bool,
    shift: bool,
) -> RowClick {
    let range = shift.then(|| anchor.as_deref().and_then(|a| span(order, a, name))).flatten();
    if let Some((lo, hi)) = range {
        for n in &order[lo..=hi] {
            marked.insert(n.clone());
        }
        return RowClick::Marked;
    }
    if ctrl || shift {
        if !marked.remove(name) {
            marked.insert(name.to_string());
        }
        *anchor = Some(name.to_string());
        return RowClick::Marked;
    }
    marked.clear();
    *anchor = Some(name.to_string());
    RowClick::Connect(name.to_string())
}

/// 두 이름의 위치를 찾아 (작은쪽, 큰쪽) 인덱스로. 하나라도 없으면 None.
fn span(order: &[String], a: &str, b: &str) -> Option<(usize, usize)> {
    let ia = order.iter().position(|n| n == a)?;
    let ib = order.iter().position(|n| n == b)?;
    Some((ia.min(ib), ia.max(ib)))
}

impl NabiApp {
    /// 선택된 세션들을 연결한다 — **자격증명이 없는 것이 있으면 먼저 확인**을 받는다.
    ///
    /// 그냥 연결하면 자격증명 없는 세션마다 접속 창이 하나씩 뜬다. 12개를 고르면 창이 12개다.
    /// 그래서 몇 개가 자동으로 붙고 몇 개가 입력을 요구하는지 미리 알려 주고 고르게 한다.
    pub(crate) fn bulk_connect(&mut self, names: &HashSet<String>) {
        let picked: Vec<SavedSession> = self
            .sessions
            .sessions
            .iter()
            .filter(|s| names.contains(&s.name))
            .cloned()
            .collect();
        if picked.is_empty() {
            return;
        }
        let need = picked.iter().filter(|s| !self.session_will_spawn(&s.kind)).count();
        if need == 0 {
            self.connect_all(picked);
        } else {
            self.bulk_ask = Some(picked); // 확인 창에서 고른다.
        }
    }

    /// 확인 없이 목록을 순서대로 연결한다.
    pub(crate) fn connect_all(&mut self, list: Vec<SavedSession>) {
        for s in list {
            self.connect_saved(s);
        }
    }

    /// 일괄 연결 확인 창 — 자동으로 붙는 수와 입력이 필요한 수를 보여 주고 고르게 한다.
    pub(crate) fn show_bulk_confirm(&mut self, ctx: &egui::Context) {
        let Some(list) = self.bulk_ask.clone() else { return };
        let lang = self.lang;
        let ready: Vec<SavedSession> = list.iter().filter(|s| self.session_will_spawn(&s.kind)).cloned().collect();
        let (total, auto) = (list.len(), ready.len());
        let (mut all, mut only_ready, mut cancel) = (false, false, false);
        crate::modal::foreground_modal(ctx, "bulk_connect", |ui| {
            ui.heading(tr(lang, "bulk.title"));
            ui.label(format!("{}: {total}", tr(lang, "bulk.count")));
            ui.label(format!("\u{2713} {}: {auto}", tr(lang, "bulk.auto")));
            ui.colored_label(
                crate::theme_ui::BROADCAST,
                format!("\u{26a0} {}: {}", tr(lang, "bulk.needlogin"), total - auto),
            );
            ui.add_space(6.0);
            ui.label(tr(lang, "bulk.hint"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(auto > 0, egui::Button::new(tr(lang, "bulk.onlyready"))).clicked() {
                    only_ready = true;
                }
                if ui.button(tr(lang, "bulk.all")).clicked() {
                    all = true;
                }
                if ui.button(tr(lang, "qc.cancel")).clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
        if all {
            self.connect_all(list);
        } else if only_ready {
            self.connect_all(ready);
        }
        if all || only_ready || cancel {
            self.bulk_ask = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_click, RowClick};
    use std::collections::HashSet;

    fn order() -> Vec<String> {
        ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_click_connects_and_clears() {
        let (mut m, mut anc) = (HashSet::from(["b".to_string()]), Some("b".to_string()));
        let r = apply_click(&mut m, &mut anc, &order(), "c", false, false);
        assert_eq!(r, RowClick::Connect("c".into()));
        assert!(m.is_empty()); // 남아 있던 선택은 비운다.
        assert_eq!(anc.as_deref(), Some("c"));
    }

    #[test]
    fn ctrl_click_toggles_without_connecting() {
        let (mut m, mut anc) = (HashSet::new(), None);
        assert_eq!(apply_click(&mut m, &mut anc, &order(), "b", true, false), RowClick::Marked);
        assert!(m.contains("b"));
        apply_click(&mut m, &mut anc, &order(), "b", true, false); // 다시 누르면 해제.
        assert!(!m.contains("b"));
    }

    #[test]
    fn shift_click_selects_range_both_directions() {
        let (mut m, mut anc) = (HashSet::new(), None);
        apply_click(&mut m, &mut anc, &order(), "b", true, false); // 앵커 b.
        apply_click(&mut m, &mut anc, &order(), "d", false, true);
        assert_eq!(m, HashSet::from(["b".into(), "c".into(), "d".into()]));
        // 역방향도 같은 범위.
        let (mut m2, mut anc2) = (HashSet::new(), Some("d".to_string()));
        apply_click(&mut m2, &mut anc2, &order(), "b", false, true);
        assert_eq!(m2, HashSet::from(["b".into(), "c".into(), "d".into()]));
    }

    #[test]
    fn shift_without_anchor_behaves_like_ctrl() {
        let (mut m, mut anc) = (HashSet::new(), None);
        assert_eq!(apply_click(&mut m, &mut anc, &order(), "c", false, true), RowClick::Marked);
        assert_eq!(m, HashSet::from(["c".to_string()]));
    }
}
