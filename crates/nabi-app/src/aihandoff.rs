//! 실패 명령 → AI 인계(2026 벤치마킹: Warp error-explain의 substrate 버전).
//!
//! 모델을 내장하지 않는다 — 사용자가 pane에서 돌리고 있는 AI CLI(claude/codex/aider…)에
//! 실패 컨텍스트(명령·종료코드·마지막 출력)를 브래킷 붙여넣기로 넘긴다. AI pane이 없으면
//! 프롬프트를 클립보드로(어디에든 붙일 수 있게). 상태바 ✗ 칩 클릭으로 연다.

use crate::app::NabiApp;
use nabi_types::PaneId;

/// 엄격한 AI CLI 판정: basename이 목록과 정확히 일치할 때만 그 이름을 돌려준다
/// (aistatus의 부분문자열 폴백 배제 — 리뷰 #10, grep 'claude ' 따위 오탐 방지).
/// 주입 대상 선정(handoff)과 AI 명령 바(aicmdbar)가 공유하는 단일 판정원.
///
/// **`aistatus::is_ai_command` 와 목록이 다른 것은 일부러다.** 저쪽은 상태바에 배지를
/// 붙일지 정하는 것이라 넓게 본다(ollama·sgpt 처럼 대화형이 아닌 것도 포함). 이쪽은
/// **글자를 밀어 넣을 상대**를 고르는 것이라 좁게 본다 — 잘못 고르면 남의 셸에
/// 프롬프트가 찍힌다.
///
/// ## 감싸서 띄운 것도 알아본다
///
/// 예전에는 **첫 토막만** 봤다. 그래서 `npx claude`·`sudo codex`·`wsl agy`·`pwsh -c claude`
/// 처럼 흔한 방법으로 띄우면 AI CLI 로 안 보였고, 명령 바도 인계도 조용히 안 됐다
/// (2026-09-01 탐침으로 확인 — 여섯 가지가 전부 걸렸다).
///
/// 이제 **껍데기(launcher)와 플래그·환경변수를 건너뛰고 처음 만나는 진짜 명령**을 본다.
/// 다만 거기서 멈춘다 — `sudo apt install claude` 처럼 뒤쪽에 이름이 섞여 있는 것까지
/// 훑으면 남의 셸에 프롬프트를 밀어 넣게 된다.
pub(crate) fn ai_command_name(cmd: &str) -> Option<&'static str> {
    // 껍데기를 벗기는 일은 `cmdbase` 한 곳에만 있다 — 상태바 배지도 같은 함수를 쓴다.
    let name = crate::cmdbase::real_command_base(cmd)?;
    // npm 패키지로 띄우면 실행 이름이 `claude-code` 다(`npx @anthropic-ai/claude-code`).
    if name == "claude-code" {
        return Some("claude");
    }
    // `warp` 는 Warp 의 독립 에이전트 CLI 다 — 2026-08 부터 어느 터미널에서나 돌고,
    // 설치 뒤 명령 이름이 `warp` 다(docs.warp.dev/agents/cli/quickstart, 2026-09-01 확인).
    ["claude", "aider", "codex", "agy", "gemini", "llm", "goose", "cursor", "warp"]
        .iter()
        .find(|n| **n == name)
        .copied()
}

/// [`ai_command_name`]의 불리언 뷰(기존 호출부 유지).
pub(crate) fn is_ai_command_strict(cmd: &str) -> bool {
    ai_command_name(cmd).is_some()
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

/// 주입 텍스트를 ASCII로 위생 처리한다(영구 규칙 "Don't Input HANGUL" — 일부 AI TUI가
/// 붙여넣은 비ASCII로 깨진다). 템플릿만 ASCII였고 **명령·출력은 그대로 주입**되던 구멍을
/// 막는다(2026-08-19 리뷰). 비ASCII 연속 구간은 물음표 하나로 접어 구조를 보존한다.
pub(crate) fn ascii_sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dropped = false;
    for ch in s.chars() {
        if ch.is_ascii() {
            out.push(ch);
            dropped = false;
        } else if !dropped {
            out.push('?');
            dropped = true;
        }
    }
    out
}

impl NabiApp {
    /// 포커스 pane의 실패 컨텍스트를 만든다(명령·종료코드·화면 마지막 30줄).
    pub(crate) fn failure_context(&self, p: PaneId) -> Option<String> {
        let exit = *self.last_exit.get(&p)?;
        if exit == 0 {
            return None;
        }
        // 종료코드·출력은 **끝난 명령**의 것이다 — 지금 다른 명령이 돌고 있어도(run_cmd)
        // 짝이 맞는 last_run_cmd를 쓴다(리뷰 2026-08-19: 실행 중 명령에 남의 종료코드가 붙던 버그).
        let cmd = self.last_run_cmd.get(&p).or_else(|| self.run_cmd.get(&p)).cloned().unwrap_or_else(|| "(unknown)".into());
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
    /// 탭·분리 창뿐 아니라 **창 안에 띄운 pane(docked_float)**도 본다 — 그쪽으로 옮겨 둔
    /// AI pane을 "AI 없음"이라고 하던 버그(리뷰 2026-08-19).
    pub(crate) fn find_ai_pane(&self, except: PaneId) -> Option<PaneId> {
        self.dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .chain(self.floating.iter().copied())
            .chain(self.docked_float.iter().copied())
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
    /// 출력·종료코드와 짝이 맞는 **끝난 명령**을 우선한다(failure_context와 같은 규칙).
    pub(crate) fn command_context(&self, p: PaneId) -> Option<String> {
        let cmd = self.last_run_cmd.get(&p).or_else(|| self.run_cmd.get(&p)).cloned()?;
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
        // 종료 코드를 함께 담는다 — 붙여 넣어 물어볼 때 성공·실패가 가장 먼저 필요하다.
        let tail = match self.last_exit.get(&p) {
            Some(0) | None => String::new(),
            Some(c) => format!("\n# exit {c}"),
        };
        Some(format!("```console\n$ {cmd}\n{}{tail}\n```", out.trim_end()))
    }

    /// 마지막 명령 블록(명령+출력+종료코드)을 클립보드에 담는다.
    ///
    /// `command_markdown`은 진작 있었는데 **아무 데서도 부르지 않았다** — 만들어 두고 안
    /// 쓰는 것은 없는 것과 같다. 팔레트·탭 메뉴에서 부를 수 있게 한다.
    pub(crate) fn copy_command_block(&mut self, ctx: &egui::Context) {
        let Some(p) = self.focused_pane() else { return };
        let key = match self.command_markdown(p) {
            Some(md) => {
                ctx.copy_text(md);
                "block.copied"
            }
            None => "block.none",
        };
        self.notify = Some((nabi_i18n::tr(self.lang, key).to_string(), std::time::Instant::now()));
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
        // 주입 직전 단일 지점에서 ASCII 위생 처리 — 명령·출력에 섞인 비ASCII를 차단한다.
        let prompt = ascii_sanitize(prompt);
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

    /// 주입 직전 위생 처리(리뷰 2026-08-19): 명령·출력에 섞인 비ASCII도 차단된다.
    #[test]
    fn sanitize_strips_non_ascii_but_keeps_structure() {
        use super::ascii_sanitize as f;
        // 공백은 ASCII라 그대로 남는다 — 단어 경계가 보존돼 AI가 구조를 읽을 수 있다.
        assert_eq!(f("error: 파일을 찾을 수 없습니다 (code 2)"), "error: ? ? ? ? (code 2)");
        assert_eq!(f("plain ascii\nline2"), "plain ascii\nline2");
        assert!(f("한글만").is_ascii());
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

#[cfg(test)]
mod nametests {
    use super::ai_command_name;

    /// 그대로 띄운 것.
    #[test]
    fn a_plain_command_is_recognised() {
        assert_eq!(ai_command_name("claude"), Some("claude"));
        assert_eq!(ai_command_name("codex --yolo"), Some("codex"));
        assert_eq!(ai_command_name(r"C:\bin\agy.exe"), Some("agy"));
    }

    /// **감싸서 띄운 것도 알아본다** — 여섯 가지 전부 예전에는 안 걸렸다.
    #[test]
    fn wrapped_launches_are_recognised() {
        for (cmd, want) in [
            ("npx claude", "claude"),
            ("sudo codex", "codex"),
            ("wsl agy", "agy"),
            ("pwsh -c claude", "claude"),
            ("uvx aider", "aider"),
            ("env FOO=1 codex", "codex"),
            ("cmd /c claude", "claude"),
            ("npx -y @anthropic-ai/claude-code", "claude"),
        ] {
            assert_eq!(ai_command_name(cmd), Some(want), "{cmd:?}");
        }
    }

    /// **비슷한 이름은 아니다.** 잘못 고르면 남의 셸에 프롬프트가 찍힌다.
    #[test]
    fn lookalikes_are_refused() {
        for c in ["claudette", "my-claude-wrapper", "claude-x", "grep claude", "git commit -m claude"] {
            assert_eq!(ai_command_name(c), None, "{c:?}");
        }
    }

    /// **껍데기 뒤 첫 명령에서 멈춘다** — 뒤쪽에 이름이 섞였다고 따라가면 안 된다.
    #[test]
    fn it_stops_at_the_first_real_command() {
        assert_eq!(ai_command_name("sudo apt install claude"), None);
        assert_eq!(ai_command_name("npx eslint --fix claude.js"), None);
    }
}
