//! 실패 명령 → AI 인계(2026 벤치마킹: Warp error-explain의 substrate 버전).
//!
//! 모델을 내장하지 않는다 — 사용자가 pane에서 돌리고 있는 AI CLI(claude/codex/aider…)에
//! 실패 컨텍스트(명령·종료코드·마지막 출력)를 브래킷 붙여넣기로 넘긴다. AI pane이 없으면
//! 프롬프트를 클립보드로(어디에든 붙일 수 있게). 상태바 ✗ 칩 클릭으로 연다.

use crate::app::NabiApp;
use nabi_types::PaneId;

/// 엄격한 AI CLI 판정: basename이 목록과 정확히 일치할 때만(aistatus의 부분문자열 폴백 배제).
/// 프롬프트를 "주입"할 대상 선정이라 오탐이 위험하다(리뷰 #10 — grep 'claude ' 따위 배제).
pub(crate) fn is_ai_command_strict(cmd: &str) -> bool {
    let base = cmd.split_whitespace().next().unwrap_or("");
    let name = base.rsplit(['\\', '/']).next().unwrap_or(base).trim_end_matches(".exe").to_ascii_lowercase();
    matches!(name.as_str(), "claude" | "aider" | "codex" | "gemini" | "llm" | "goose" | "cursor")
}

/// 실패 컨텍스트 프롬프트(순수) — AI가 바로 진단할 수 있는 최소 정보.
///
/// **주입 텍스트는 항상 ASCII 영어**(사용자 긴급 보고 2026-08-17 "Don't Input HANGUL"):
/// Windows의 일부 AI TUI(claude CLI 등)가 붙여넣은 한글을 깨뜨리거나 입력을 막는 사례가
/// 있어, pane에 "타이핑되는" 텍스트에는 한글을 절대 넣지 않는다(UI 라벨은 3어 유지).
/// AI는 영어 프롬프트에도 한국어로 답할 수 있으므로 기능 손실이 없다.
pub(crate) fn failure_prompt(_lang: nabi_i18n::Lang, cmd: &str, exit: i32, tail: &str) -> String {
    format!(
        "This command failed. Explain why and how to fix it.\n\nCommand: `{cmd}`\nExit code: {exit}\n\nOutput:\n```\n{}\n```",
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
        let cmd = self.run_cmd.get(&p).or_else(|| self.last_run_cmd.get(&p)).cloned().unwrap_or_else(|| "(unknown)".into());
        // 셸 통합(OSC 133)이 있으면 "그 명령의 실제 출력"을, 없으면 화면 마지막 30줄 폴백.
        let tail = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).map(|v| v.model.clone()))
            .and_then(|md| md.lock().ok().map(|m| m.last_command_output().unwrap_or_else(|| m.visible_bottom_text(30))))
            .unwrap_or_default();
        Some(failure_prompt(self.lang, &cmd, exit, &tail))
    }

    /// AI CLI가 돌고 있는 다른 pane(첫 번째)을 찾는다.
    pub(crate) fn find_ai_pane(&self, except: PaneId) -> Option<PaneId> {
        self.dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .chain(self.floating.iter().copied())
            .find(|p| {
                *p != except
                    && self.run_cmd.get(p).is_some_and(|c| is_ai_command_strict(c))
            })
    }

    /// 실패 컨텍스트를 AI pane에 넘긴다(상태바 ✗ 칩).
    pub(crate) fn handoff_failure_to_ai(&mut self, src: PaneId, ai: PaneId) {
        if let Some(prompt) = self.failure_context(src) {
            self.inject_prompt(ai, &prompt);
        }
    }

    /// 마지막 명령 컨텍스트(성공 포함) — 실패 여부와 무관하게 "이 결과 봐줘" 동선(팔레트).
    pub(crate) fn command_context(&self, p: PaneId) -> Option<String> {
        let cmd = self.run_cmd.get(&p).or_else(|| self.last_run_cmd.get(&p)).cloned()?;
        let exit = self.last_exit.get(&p).copied().unwrap_or(0);
        let tail = self.pane_cmd_output(p)?;
        // 주입 텍스트는 항상 ASCII 영어(failure_prompt 주석 참조 — "Don't Input HANGUL").
        Some(format!(
            "Here's a command I ran and its output. Take a look.\n\nCommand: `{cmd}`\nExit code: {exit}\n\nOutput:\n```\n{}\n```",
            tail.trim_end()
        ))
    }

    /// 마지막 명령+출력을 마크다운 코드블록으로(클립보드용 — AI 채팅/이슈에 붙여넣기).
    pub(crate) fn command_markdown(&self, p: PaneId) -> Option<String> {
        let cmd = self.run_cmd.get(&p).or_else(|| self.last_run_cmd.get(&p)).cloned()?;
        let out = self.pane_cmd_output(p)?;
        Some(format!("```console\n$ {cmd}\n{}\n```", out.trim_end()))
    }

    /// pane의 마지막 명령 출력(OSC 133 우선, 폴백=화면 마지막 30줄).
    fn pane_cmd_output(&self, p: PaneId) -> Option<String> {
        self.orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).map(|v| v.model.clone()))
            .and_then(|md| md.lock().ok().map(|m| m.last_command_output().unwrap_or_else(|| m.visible_bottom_text(30))))
    }

    /// 프롬프트를 AI pane에 브래킷 붙여넣기로 주입하고 그 탭을 활성화한다(공용).
    pub(crate) fn inject_prompt(&mut self, ai: PaneId, prompt: &str) {
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
    fn strict_rejects_substring_mentions() {
        use super::is_ai_command_strict as f;
        assert!(f("claude --resume") && f(r"C:\Users\u\claude.exe") && f("codex"));
        assert!(!f("grep -r 'claude ' src/"), "부분문자열 언급은 배제");
        assert!(!f("tail -f claude.log"));
        assert!(!f(""));
    }

    #[test]
    fn prompt_contains_essentials() {
        let p = failure_prompt(nabi_i18n::Lang::Ko, "cargo build", 101, "error[E0308]: mismatched types\n");
        assert!(p.contains("`cargo build`"));
        assert!(p.contains("101"));
        assert!(p.contains("E0308"));
        assert!(p.ends_with("```"), "code fence closed: {p}");
        assert!(p.is_ascii(), "주입 프롬프트는 ASCII 전용(Don't Input HANGUL): {p}");
    }
}
