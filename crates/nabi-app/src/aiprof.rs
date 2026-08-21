//! AI 터미널 프로필(세션▸새 AI 터미널) — 프리셋 스위치 테이블·명령줄 조립·실행.
//!
//! 프로필은 config `terminal.ai_profiles`(SSOT: args Vec)에 저장된다. 설정 UI의
//! 체크박스는 args 목록을 넣고 빼는 편집기일 뿐이다(aiprofileui.rs).

use crate::app::NabiApp;
use nabi_config::AiProfileCfg;

/// CLI별 불리언 스위치 프리셋: (스위치, 설명 i18n 키). 스위치명은 제품 옵션이라 번역하지
/// 않고, 설명을 체크박스 아래 흐린 글씨로 보여 선택을 돕는다(사용자 요청 2026-08-18).
/// 값이 필요한 옵션(--model 등)은 "추가 인자"로 넣는다. 미지 CLI는 빈 목록.
pub(crate) fn preset_switches(cmd: &str) -> &'static [(&'static str, &'static str)] {
    match cmd {
        // 2026-08-18 공식 CLI 레퍼런스(code.claude.com/docs/en/cli-reference)로 검증.
        "claude" => &[
            ("--dangerously-skip-permissions", "aiopt.claude.skipperm"),
            ("--allow-dangerously-skip-permissions", "aiopt.claude.allowskip"),
            ("--continue", "aiopt.claude.continue"),
            ("--resume", "aiopt.claude.resume"),
            ("--ide", "aiopt.claude.ide"),
            ("--chrome", "aiopt.claude.chrome"),
            ("--bare", "aiopt.claude.bare"),
            ("--fork-session", "aiopt.claude.fork"),
            ("--remote-control", "aiopt.claude.rc"),
        ],
        "codex" => &[
            ("--full-auto", "aiopt.codex.fullauto"),
            ("--dangerously-bypass-approvals-and-sandbox", "aiopt.codex.bypass"),
            ("--search", "aiopt.codex.search"),
            ("--oss", "aiopt.codex.oss"),
        ],
        // Antigravity CLI(`agy`) — Gemini CLI의 후속. Gemini CLI는 2026-06-18 서비스 종료라
        // `gemini` 선택지는 실행조차 되지 않았다(사용자 피드백 2026-08-21).
        // 슬래시 명령은 공식 레퍼런스(antigravity.google/docs/cli/reference), 실행 스위치는
        // 커뮤니티 치트시트 기준 — 값이 필요한 --model/--effort/--mode는 "추가 인자"로 넣는다.
        "antigravity" => &[
            ("--dangerously-skip-permissions", "aiopt.agy.skipperm"),
            ("--sandbox", "aiopt.agy.sandbox"),
        ],
        "aider" => &[
            ("--yes-always", "aiopt.aider.yes"),
            ("--watch-files", "aiopt.aider.watch"),
            ("--no-auto-commits", "aiopt.aider.nocommit"),
            ("--cache-prompts", "aiopt.aider.cache"),
            ("--dry-run", "aiopt.aider.dryrun"),
            ("--vim", "aiopt.aider.vim"),
        ],
        _ => &[],
    }
}

/// 프리셋 스위치 이름 포함 여부(설명 키 무시) — extra/preset 분리 판정용.
fn is_preset(cmd: &str, arg: &str) -> bool {
    preset_switches(cmd).iter().any(|(sw, _)| *sw == arg)
}

/// 설정 UI의 CLI 종류 선택지(마지막 "custom"은 자유 입력).
pub(crate) const CLI_CHOICES: [&str; 5] = ["claude", "codex", "antigravity", "aider", "custom"];

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
    args.iter().filter(|a| !is_preset(cmd, a)).cloned().collect::<Vec<_>>().join(" ")
}

/// 자유 입력 한 줄을 공백 분리해 args의 비프리셋 부분을 교체한다(프리셋 체크는 유지).
pub(crate) fn set_extra_args(args: &mut Vec<String>, cmd: &str, text: &str) {
    args.retain(|a| is_preset(cmd, a));
    args.extend(text.split_whitespace().map(str::to_string));
}

/// 선택지 id → 실제 실행 파일 이름. 대부분 같지만 Antigravity는 표시명과 명령이 다르다
/// (제품명 "Antigravity", 명령 `agy`). 이 매핑이 없으면 존재하지 않는 명령을 실행한다.
pub(crate) fn exec_name(choice: &str) -> &str {
    match choice {
        "antigravity" => "agy",
        other => other,
    }
}

/// 프로필 → 셸에 주입할 명령줄. 주입 텍스트는 ASCII 전용 규칙이라 비ASCII면 None.
pub(crate) fn command_line(p: &AiProfileCfg) -> Option<String> {
    let mut line = exec_name(p.cmd.trim()).to_string();
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
