//! SSH 세션 복원 시 AI CLI만 골라 "이어서" 명령으로 되살린다.
//!
//! 원격에서 임의 명령을 되돌려 실행하면 위험하다 — 그래서 저장한 명령을 그대로 쓰지 않고,
//! **아는 AI CLI만** 고정된 재개 명령으로 바꿔 넣는다(허용 목록). 목록에 없으면 아무것도 안 한다.

use crate::app::NabiApp;

impl NabiApp {
    /// SSH에서는 임의 명령을 저장하지 않고 AI CLI 허용 목록만 고정 재개 명령으로 바꾼다.
    pub(crate) fn saved_ssh_ai_command(&self, p: nabi_types::PaneId) -> Option<String> {
        if !self.config.terminal.restore_ssh_ai_command {
            return None;
        }
        if let Some(cmd) = self.run_cmd.get(&p).and_then(|s| ai_resume_command(s)) {
            return Some(cmd);
        }
        let title = self
            .orch
            .panes
            .read()
            .ok()
            .and_then(|m| m.get(&p).map(|v| v.title.clone()))?;
        ai_resume_from_title(&title)
    }
}

fn ai_resume_command(command: &str) -> Option<String> {
    let first = command
        .split_whitespace()
        .next()?
        .rsplit(['/', '\\'])
        .next()?;
    let base = first
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .to_ascii_lowercase();
    match base.as_str() {
        "claude" => Some("claude --continue".into()),
        "codex" => Some("codex resume --last".into()),
        "agy" => Some("agy".into()),
        _ => None,
    }
}

fn ai_resume_from_title(title: &str) -> Option<String> {
    let t = title.to_ascii_lowercase();
    if t.contains("claude") {
        Some("claude --continue".into())
    } else if t.contains("codex") {
        Some("codex resume --last".into())
    } else if t.contains("antigravity") || t.split_whitespace().any(|s| s == "agy") {
        Some("agy".into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{ai_resume_command, ai_resume_from_title};

    #[test]
    fn remote_ai_resume_is_strictly_allowlisted() {
        assert_eq!(
            ai_resume_command("codex --model x").as_deref(),
            Some("codex resume --last")
        );
        assert_eq!(
            ai_resume_command("/usr/bin/claude").as_deref(),
            Some("claude --continue")
        );
        assert_eq!(ai_resume_command("rm -rf x"), None);
        assert_eq!(
            ai_resume_from_title("Codex CLI").as_deref(),
            Some("codex resume --last")
        );
    }
}
