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

/// 로컬 pane 복원 명령: 훅이 보고한 세션 ID가 있으면 `--resume <id>`(정확), 없으면
/// 근사 재개(`--continue`), AI CLI가 아니면 원문 유지(A6).
///
/// claude는 **사용자 플래그를 보존**한다(사용자 보고 2026-08-18: `claude
/// --dangerously-skip-permissions --continue`가 복원 후 `--continue`만 남던 문제).
/// 로컬은 AI가 아닌 명령도 원문 그대로 복원하므로 플래그를 지울 이유가 없다 —
/// 재개 플래그(`--continue`/`--resume [id]`)만 정리해 교체/보충한다.
/// codex는 `resume`가 서브커맨드라 플래그 위치 규칙이 달라 현행(고정형)을 유지한다.
/// SSH 쪽(saved_ssh_ai_command)은 보안 허용 목록 설계라 그대로 고정형이다.
pub(crate) fn local_resume_command(command: &str, session: Option<&str>) -> String {
    let first = command.split_whitespace().next().unwrap_or("");
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first).to_ascii_lowercase();
    let base = base.trim_end_matches(".exe").trim_end_matches(".cmd").trim_end_matches(".bat");
    match (base, session) {
        ("claude", sid) => claude_resume_preserving_flags(command, sid),
        ("codex", Some(id)) => format!("codex resume {id}"),
        ("codex", None) => "codex resume --last".into(),
        _ => command.to_string(),
    }
}

/// claude 명령줄에서 기존 재개 관련 토큰(`--continue`/`-c`/`--resume [id]`/`-r [id]`)만
/// 걷어내고 나머지 플래그를 보존한 뒤, 정확 재개(`--resume <id>`) 또는 근사(`--continue`)를 붙인다.
fn claude_resume_preserving_flags(command: &str, session: Option<&str>) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut it = command.split_whitespace().skip(1).peekable();
    while let Some(tok) = it.next() {
        match tok {
            "--continue" | "-c" => {}
            "--resume" | "-r" => {
                // 다음 토큰이 값(세션 ID)이면 함께 제거(플래그 시작이면 유지).
                if it.peek().is_some_and(|n| !n.starts_with('-')) {
                    it.next();
                }
            }
            _ => kept.push(tok),
        }
    }
    let mut out = String::from("claude");
    for t in kept {
        out.push(' ');
        out.push_str(t);
    }
    match session {
        Some(id) => {
            out.push_str(" --resume ");
            out.push_str(id);
        }
        None => out.push_str(" --continue"),
    }
    out
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

    /// 로컬 복원: 세션 ID가 있으면 정확 재개, 없으면 근사, AI가 아니면 원문(A6).
    #[test]
    fn local_resume_prefers_session_id() {
        use super::local_resume_command as f;
        assert_eq!(f("claude", Some("abc-123")), "claude --resume abc-123");
        assert_eq!(f("claude --continue", None), "claude --continue");
        assert_eq!(f(r"C:\npm\codex.cmd resume --last", Some("s9")), "codex resume s9");
        assert_eq!(f("codex", None), "codex resume --last");
        assert_eq!(f("cargo watch", Some("x")), "cargo watch", "AI 아니면 세션 무시·원문 유지");
    }

    /// 사용자 보고(2026-08-18): claude 플래그가 복원에서 사라지면 안 된다.
    #[test]
    fn local_resume_preserves_claude_flags() {
        use super::local_resume_command as f;
        assert_eq!(
            f("claude --dangerously-skip-permissions --continue", None),
            "claude --dangerously-skip-permissions --continue"
        );
        assert_eq!(
            f("claude --dangerously-skip-permissions --continue", Some("s1")),
            "claude --dangerously-skip-permissions --resume s1",
            "정확 재개로 승격돼도 다른 플래그는 유지"
        );
        assert_eq!(
            f("claude --resume old-id --model opus", None),
            "claude --model opus --continue",
            "묵은 세션 ID는 제거(끝났을 수 있음), 값 플래그는 보존"
        );
        assert_eq!(f(r"C:\bin\claude.exe -c --verbose", Some("z")), "claude --verbose --resume z");
    }

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
