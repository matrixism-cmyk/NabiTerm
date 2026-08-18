//! AI 터미널 프로필(세션▸새 AI 터미널) — 프리셋 스위치 테이블·명령줄 조립·실행.
//!
//! 프로필은 config `terminal.ai_profiles`(SSOT: args Vec)에 저장된다. 설정 UI의
//! 체크박스는 args 목록을 넣고 빼는 편집기일 뿐이다(aiprofileui.rs).

use crate::app::NabiApp;
use nabi_config::AiProfileCfg;

/// CLI별 잘 알려진 스위치 프리셋(설정 UI 체크박스). 제품 옵션명이라 번역하지 않는다.
/// 알려지지 않은 CLI(agy 등)는 빈 목록 — 추가 인자 입력으로 쓴다.
pub(crate) fn preset_switches(cmd: &str) -> &'static [&'static str] {
    match cmd {
        "claude" => &["--dangerously-skip-permissions", "--continue", "--resume", "--verbose"],
        "codex" => &["--full-auto", "--dangerously-bypass-approvals-and-sandbox", "--search"],
        "gemini" => &["--yolo", "--sandbox"],
        "aider" => &["--yes-always", "--watch-files"],
        _ => &[],
    }
}

/// 설정 UI의 CLI 종류 선택지(마지막 "custom"은 자유 입력).
pub(crate) const CLI_CHOICES: [&str; 5] = ["claude", "codex", "gemini", "aider", "custom"];

/// args에서 프리셋 스위치를 켜고 끈다(중복 없이, 순서 유지).
pub(crate) fn toggle_arg(args: &mut Vec<String>, sw: &str, on: bool) {
    let has = args.iter().any(|a| a == sw);
    if on && !has {
        args.push(sw.to_string());
    } else if !on && has {
        args.retain(|a| a != sw);
    }
}

/// 프리셋에 없는 인자들(자유 입력분)을 한 줄로 합친다(설정 UI 표시용).
pub(crate) fn extra_args_string(args: &[String], cmd: &str) -> String {
    let presets = preset_switches(cmd);
    args.iter().filter(|a| !presets.contains(&a.as_str())).cloned().collect::<Vec<_>>().join(" ")
}

/// 자유 입력 한 줄을 공백 분리해 args의 비프리셋 부분을 교체한다(프리셋 체크는 유지).
pub(crate) fn set_extra_args(args: &mut Vec<String>, cmd: &str, text: &str) {
    let presets = preset_switches(cmd);
    args.retain(|a| presets.contains(&a.as_str()));
    args.extend(text.split_whitespace().map(str::to_string));
}

/// 프로필 → 셸에 주입할 명령줄. 주입 텍스트는 ASCII 전용 규칙이라 비ASCII면 None.
pub(crate) fn command_line(p: &AiProfileCfg) -> Option<String> {
    let mut line = p.cmd.trim().to_string();
    if line.is_empty() {
        return None;
    }
    for a in &p.args {
        let a = a.trim();
        if !a.is_empty() {
            line.push(' ');
            line.push_str(a);
        }
    }
    line.is_ascii().then_some(line)
}

impl NabiApp {
    /// 프로필 i번으로 새 AI 터미널을 연다(셸 스폰 후 on_connect로 CLI 실행 — 복원과 동일 경로).
    pub(crate) fn spawn_ai_profile(&mut self, idx: usize) {
        let Some(p) = self.config.terminal.ai_profiles.get(idx).cloned() else { return };
        let Some(line) = command_line(&p) else {
            // 주입 텍스트 ASCII 전용 규칙(2026-08-17 "Don't Input HANGUL") 위반 — 실행 대신 안내.
            self.notify = Some((nabi_i18n::tr(self.lang, "aiprof.ascii").to_string(), std::time::Instant::now()));
            return;
        };
        let shell = if p.shell.is_empty() {
            crate::workspace::shell_from_str(&self.config.terminal.default_shell)
        } else {
            crate::workspace::shell_from_str(&p.shell)
        };
        self.spawn_local_cwd(shell, Some(line), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_extra_roundtrip() {
        let mut args = vec!["--continue".to_string(), "-m".into(), "opus".into()];
        toggle_arg(&mut args, "--dangerously-skip-permissions", true);
        toggle_arg(&mut args, "--dangerously-skip-permissions", true); // 중복 방지.
        assert_eq!(args.iter().filter(|a| *a == "--dangerously-skip-permissions").count(), 1);
        toggle_arg(&mut args, "--continue", false);
        assert!(!args.contains(&"--continue".to_string()));
        assert_eq!(extra_args_string(&args, "claude"), "-m opus");
        set_extra_args(&mut args, "claude", "--model sonnet");
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()), "프리셋 체크 유지");
        assert_eq!(extra_args_string(&args, "claude"), "--model sonnet");
    }

    #[test]
    fn command_line_is_ascii_only() {
        let p = nabi_config::AiProfileCfg {
            name: "야간".into(), // 이름은 한글 가능 — 주입되지 않는다.
            shell: String::new(),
            cmd: "claude".into(),
            args: vec!["--dangerously-skip-permissions".into()],
        };
        assert_eq!(command_line(&p).as_deref(), Some("claude --dangerously-skip-permissions"));
        let bad = nabi_config::AiProfileCfg { args: vec!["한글인자".into()], ..p.clone() };
        assert_eq!(command_line(&bad), None, "비ASCII 인자는 주입 금지(Don't Input HANGUL)");
        let empty = nabi_config::AiProfileCfg { cmd: "  ".into(), ..p };
        assert_eq!(command_line(&empty), None);
    }
}
