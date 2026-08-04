//! 출력 트리거 알림(MobaXterm trigger/Warp식) — 새 터미널 출력에 사용자 지정 패턴이 나타나면
//! 토스트 + (비포커스 시) 작업표시줄 주의 환기. 빌드 완료·에러 등을 다른 작업 중에도 인지(AI 코딩).
//! line_marker 델타로 신규 줄만 검사(세션 로깅과 같은 패턴).

use crate::app::NabiApp;
use nabi_types::PaneId;
use std::time::{Duration, Instant};

impl NabiApp {
    /// ~1초마다 모든 pane의 신규 출력 줄을 검사해 패턴 일치 시 알림. 패턴 없으면 즉시 반환.
    pub(crate) fn check_output_alerts(&mut self, ctx: &egui::Context) {
        if self.config.terminal.alert_patterns.iter().all(|p| p.trim().is_empty()) {
            return;
        }
        if self.alert_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.alert_check = Instant::now();
        let pats: Vec<String> = self.config.terminal.alert_patterns.iter()
            .map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect();
        // (pane, 신규줄 텍스트) 수집(가드 분리).
        let mut hits: Vec<(PaneId, String)> = Vec::new();
        if let Ok(panes) = self.orch.panes.read() {
            for (id, v) in panes.iter() {
                let Ok(md) = v.model.lock() else { continue };
                let cur = md.line_marker();
                let last = *self.alert_marks.get(id).unwrap_or(&cur); // 처음 보는 pane은 현재부터.
                if cur > last {
                    let text = md.lines_abs_text(last, cur).join("\n").to_lowercase();
                    if let Some(p) = pats.iter().find(|p| text.contains(p.as_str())) {
                        hits.push((*id, p.clone()));
                    }
                }
                self.alert_marks.insert(*id, cur);
            }
        }
        if let Some((_, pat)) = hits.first() {
            self.notify = Some((format!("\u{1f514} {pat}"), Instant::now()));
            if !ctx.input(|i| i.focused) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
            }
        }
    }
}
