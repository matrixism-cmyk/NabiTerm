//! AI 사용량 대시보드 창(B2) + pane별 에이전트 상태 목록(B1) — aistatus.rs에서 분리(라인 한도).
//! 순수 집계·상태 판정은 [`crate::aistatus`](aggregate/agent_state/context_tier 등)를 재사용.

use crate::aistatus::{aggregate, context_tier, is_ai_command, parse_cost, parse_token_usage};

/// 대시보드 한 행: (pane, 제목, 상태0/1/2, 컨텍스트율, 비용$).
type AiRow = (nabi_types::PaneId, String, u8, Option<f32>, Option<f32>);

impl crate::app::NabiApp {
    /// AI 사용량 대시보드(팔레트/보기 메뉴 토글) — 전 pane 비용·세션·최대 컨텍스트 합산 +
    /// pane별 상태(working/idle/blocked) 목록(클릭 시 해당 탭으로 포커스).
    pub(crate) fn ai_dashboard(&mut self, ctx: &egui::Context) {
        if !self.ai_dash_open {
            return;
        }
        let lang = self.lang;
        let agg = aggregate(self.pane_status.values());
        // pane별 상태 행 사전계산(borrow 회피 — closure 밖에서 self 읽기).
        let mut rows: Vec<AiRow> = Vec::new();
        if let Ok(names) = self.orch.panes.read() {
            for (p, st) in &self.pane_status {
                if st.is_empty() { continue; }
                let title = names.get(p).map(|v| v.title.clone()).unwrap_or_default();
                let running = self.cmd_start.contains_key(p) || self.run_cmd.get(p).map(|c| is_ai_command(c)).unwrap_or(false);
                let cost = st.get("cost").and_then(|v| parse_cost(v));
                rows.push((*p, title, self.merged_agent_state(*p, st, running), st.get("tokens").and_then(|t| parse_token_usage(t)), cost));
            }
            // 화면 감지로만 잡힌 에이전트(훅 미설치 — codex 등)도 목록에 올린다.
            let empty = std::collections::BTreeMap::new();
            for p in self.agent_watch.state.keys() {
                if self.pane_status.get(p).is_some_and(|st| !st.is_empty()) { continue; }
                let title = names.get(p).map(|v| v.title.clone()).unwrap_or_default();
                rows.push((*p, title, self.merged_agent_state(*p, &empty, false), None, None));
            }
        }
        // 입력 대기(2) → 실행 중(1) → 완료·미확인(3) → 대기(0), 같은 상태는 제목순.
        let rank = |s: u8| match s { 2 => 3, 1 => 2, 3 => 1, _ => 0 };
        rows.sort_by_key(|r| (std::cmp::Reverse(rank(r.2)), r.1.to_lowercase()));
        let mut focus: Option<nabi_types::PaneId> = None;
        let mut open = true;
        egui::Window::new(nabi_i18n::tr(lang, "ai.dashboard"))
            .open(&mut open).collapsible(false).resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 90.0])
            .show(ctx, |ui| {
                if agg.panes == 0 && rows.is_empty() {
                    ui.label(nabi_i18n::tr(lang, "ai.noagents"));
                    return;
                }
                ui.label(format!("\u{1f916} {}: {}", nabi_i18n::tr(lang, "ai.agents"), agg.panes));
                ui.label(format!("\u{1f4b2} {}: ${:.2}", nabi_i18n::tr(lang, "ai.totalcost"), agg.total_cost));
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", nabi_i18n::tr(lang, "ai.maxctx")));
                    let fill = match context_tier(agg.max_gauge) { 2 => crate::theme_ui::ERR, 1 => crate::theme_ui::BROADCAST, _ => crate::theme_ui::ACCENT };
                    ui.add(egui::ProgressBar::new(agg.max_gauge).desired_width(80.0).fill(fill).text(format!("{:.0}%", agg.max_gauge * 100.0)));
                });
                ui.separator();
                for (p, title, state, ctx, cost) in &rows {
                    ui.horizontal(|ui| {
                        let (dot, key) = match state {
                            2 => (crate::theme_ui::BROADCAST, "ai.state.blocked"),
                            1 => (crate::theme_ui::OK, "ai.state.working"),
                            3 => (crate::theme_ui::ACCENT, "ai.state.done"),
                            _ => (egui::Color32::GRAY, "ai.state.idle"),
                        };
                        ui.colored_label(dot, "\u{25cf}").on_hover_text(nabi_i18n::tr(lang, key));
                        if ui.selectable_label(false, title).clicked() { focus = Some(*p); }
                        // 상태별 라벨(B7): status set label.working "refactoring auth" 식 발행.
                        let sname = match state { 2 => "blocked", 1 => "working", 3 => "done", _ => "idle" };
                        if let Some(lbl) = self.pane_status.get(p).and_then(|m| m.get(&format!("label.{sname}"))) {
                            ui.weak(lbl);
                        }
                        if let Some(g) = ctx { ui.weak(format!("{:.0}%", g * 100.0)); }
                        if let Some(c) = cost { ui.weak(format!("\u{1f4b2}{c:.2}")); }
                    });
                }
            });
        if let Some(p) = focus {
            if let Some(loc) = self.dock.find_tab(&p) { self.dock.set_active_tab(loc); }
        }
        if !open {
            self.ai_dash_open = false;
        }
    }
}
