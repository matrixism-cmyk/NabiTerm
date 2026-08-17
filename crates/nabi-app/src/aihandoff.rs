//! 실패 명령 → AI 인계(2026 벤치마킹: Warp error-explain의 substrate 버전).
//!
//! 모델을 내장하지 않는다 — 사용자가 pane에서 돌리고 있는 AI CLI(claude/codex/aider…)에
//! 실패 컨텍스트(명령·종료코드·마지막 출력)를 브래킷 붙여넣기로 넘긴다. AI pane이 없으면
//! 프롬프트를 클립보드로(어디에든 붙일 수 있게). 상태바 ✗ 칩 클릭으로 연다.

use crate::app::NabiApp;
use nabi_types::PaneId;

/// 실패 컨텍스트 프롬프트(순수) — AI가 바로 진단할 수 있는 최소 정보.
pub(crate) fn failure_prompt(cmd: &str, exit: i32, tail: &str) -> String {
    format!(
        "다음 명령이 실패했어. 원인을 설명하고 고치는 방법을 알려줘.\n\n명령: `{cmd}`\n종료코드: {exit}\n\n마지막 출력:\n```\n{}\n```",
        tail.trim_end()
    )
}

impl NabiApp {
    /// 포커스 pane의 실패 컨텍스트를 만든다(명령·종료코드·화면 마지막 30줄).
    pub(crate) fn failure_context(&self, p: PaneId) -> Option<String> {
        let exit = *self.last_exit.get(&p)?;
        if exit == 0 {
            return None;
        }
        let cmd = self.run_cmd.get(&p).cloned().unwrap_or_else(|| "(알 수 없음)".into());
        let tail = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).map(|v| v.model.clone()))
            .and_then(|md| md.lock().ok().map(|m| m.visible_bottom_text(30)))
            .unwrap_or_default();
        Some(failure_prompt(&cmd, exit, &tail))
    }

    /// AI CLI가 돌고 있는 다른 pane(첫 번째)을 찾는다.
    pub(crate) fn find_ai_pane(&self, except: PaneId) -> Option<PaneId> {
        self.dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .chain(self.floating.iter().copied())
            .find(|p| {
                *p != except
                    && self.run_cmd.get(p).is_some_and(|c| crate::aistatus::is_ai_command(c))
            })
    }

    /// 실패 컨텍스트를 AI pane에 브래킷 붙여넣기로 주입하고 그 탭을 활성화한다.
    pub(crate) fn handoff_failure_to_ai(&mut self, src: PaneId, ai: PaneId) {
        let Some(prompt) = self.failure_context(src) else { return };
        // 여러 줄 프롬프트가 줄마다 제출되지 않게 브래킷 붙여넣기로 감싼다(AI TUI는 지원).
        let mut data = Vec::with_capacity(prompt.len() + 16);
        data.extend_from_slice(b"\x1b[200~");
        data.extend_from_slice(prompt.as_bytes());
        data.extend_from_slice(b"\x1b[201~\r");
        self.orch.send(nabi_proto::Command::WriteInput { pane: ai, data: bytes::Bytes::from(data) });
        if let Some(loc) = self.dock.find_tab(&ai) {
            let _ = self.dock.set_active_tab(loc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::failure_prompt;

    #[test]
    fn prompt_contains_essentials() {
        let p = failure_prompt("cargo build", 101, "error[E0308]: mismatched types\n");
        assert!(p.contains("`cargo build`"));
        assert!(p.contains("101"));
        assert!(p.contains("E0308"));
        assert!(p.ends_with("```"), "코드펜스 닫힘: {p}");
    }
}
