//! AI 터미널 프로필(세션▸새 AI 터미널) — 프리셋 스위치 테이블·명령줄 조립·실행.
//!
//! 프로필은 config `terminal.ai_profiles`(SSOT: args Vec)에 저장된다. 설정 UI의
//! 체크박스는 args 목록을 넣고 빼는 편집기일 뿐이다(aiprofileui.rs).

use crate::app::NabiApp;
use nabi_config::AiProfileCfg;

/// CLI별 불리언 스위치 프리셋: (스위치, 설명 i18n 키). 스위치명은 제품 옵션이라 번역하지
/// 않고, 설명을 체크박스 아래 흐린 글씨로 보여 선택을 돕는다(사용자 요청 2026-08-18).
/// 값이 필요한 옵션(--model 등)은 "추가 인자"로 넣는다. 미지 CLI는 빈 목록.
///
/// **2026-08-25 전수 재검증**(사용자 요청). claude 2.1.241 · codex 0.149.1은 이 PC에 설치된
/// 바이너리의 `--help`를 직접 읽어 대조했고, 설치돼 있지 않은 aider·agy는 공식/레퍼런스
/// 문서로 확인했다. 이때 codex `--full-auto`가 **없어진 것**을 발견했다 — 그대로 두면 프로필이
/// 실행조차 되지 않는다(`error: unexpected argument`). 없어진 옵션은 [`deprecated_switches`]가
/// 맡아 사용자에게 알리고 고쳐 준다.
pub(crate) fn preset_switches(cmd: &str) -> &'static [(&'static str, &'static str)] {
    match cmd {
        "claude" => &[
            ("--dangerously-skip-permissions", "aiopt.claude.skipperm"),
            ("--allow-dangerously-skip-permissions", "aiopt.claude.allowskip"),
            ("--continue", "aiopt.claude.continue"),
            ("--resume", "aiopt.claude.resume"),
            ("--fork-session", "aiopt.claude.fork"),
            ("--ide", "aiopt.claude.ide"),
            ("--chrome", "aiopt.claude.chrome"),
            ("--remote-control", "aiopt.claude.rc"),
            ("--worktree", "aiopt.claude.worktree"),
            ("--background", "aiopt.claude.bg"),
            ("--bare", "aiopt.claude.bare"),
            ("--safe-mode", "aiopt.claude.safemode"),
            ("--verbose", "aiopt.claude.verbose"),
        ],
        "codex" => &[
            ("--search", "aiopt.codex.search"),
            ("--oss", "aiopt.codex.oss"),
            ("--approve-for-me", "aiopt.codex.approve"),
            ("--dangerously-bypass-approvals-and-sandbox", "aiopt.codex.bypass"),
            ("--dangerously-bypass-hook-trust", "aiopt.codex.hooktrust"),
            ("--no-alt-screen", "aiopt.codex.noalt"),
            ("--strict-config", "aiopt.codex.strict"),
        ],
        // Antigravity CLI(`agy`) — Gemini CLI의 후속. Gemini CLI는 2026-06-18 서비스 종료라
        // `gemini` 선택지는 실행조차 되지 않았다(사용자 피드백 2026-08-21).
        // 이 PC에 설치돼 있지 않아 문서로만 확인했다 — 확실히 하려면 '설치된 CLI로 확인'을 쓴다.
        "antigravity" => &[
            ("--dangerously-skip-permissions", "aiopt.agy.skipperm"),
            ("--sandbox", "aiopt.agy.sandbox"),
            ("--continue", "aiopt.agy.continue"),
            ("--disable-slash-commands", "aiopt.agy.noslash"),
        ],
        // gemini 설명도 이미 번역돼 있었는데 표에 없었다 — 옵션 화면이 gemini 를 몰랐다.
        // 이름은 Gemini CLI 문서 기준(--yolo·--sandbox·--checkpointing·--debug).
        "gemini" => &[
            ("--yolo", "aiopt.gemini.yolo"),
            ("--sandbox", "aiopt.gemini.sandbox"),
            ("--checkpointing", "aiopt.gemini.checkpoint"),
            ("--debug", "aiopt.gemini.debug"),
        ],
        "aider" => &[
            ("--yes-always", "aiopt.aider.yes"),
            ("--watch-files", "aiopt.aider.watch"),
            ("--no-auto-commits", "aiopt.aider.nocommit"),
            ("--cache-prompts", "aiopt.aider.cache"),
            ("--auto-test", "aiopt.aider.autotest"),
            ("--restore-chat-history", "aiopt.aider.restore"),
            ("--subtree-only", "aiopt.aider.subtree"),
            ("--no-check-update", "aiopt.aider.nocheck"),
            ("--dry-run", "aiopt.aider.dryrun"),
            ("--vim", "aiopt.aider.vim"),
        ],
        _ => &[],
    }
}

/// 그 CLI에서 **없어진** 옵션: (죽은 스위치, 대신 쓸 것 또는 빈 문자열, 설명 i18n 키).
///
/// 표에서 지우는 것만으로는 부족하다 — 이미 저장된 프로필의 args에 남아 있으면 그대로
/// 주입돼 CLI가 통째로 실행에 실패한다. 그래서 죽은 것을 기억해 두고 편집 화면에서 알린다.
pub(crate) fn deprecated_switches(cmd: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match cmd {
        // codex 0.149.1에서 제거됨(직접 확인). 같은 효과는 샌드박스 모드로 지정한다.
        "codex" => &[
            ("--full-auto", "--sandbox workspace-write", "aiopt.dead.codex.fullauto"),
            ("--dangerously-skip-permissions", "--dangerously-bypass-approvals-and-sandbox", "aiopt.dead.codex.skipperm"),
        ],
        _ => &[],
    }
}

/// args에서 죽은 스위치를 찾아 (죽은 것, 대체, 설명 키) 목록으로 돌려준다.
pub(crate) fn stale_in(args: &[String], cmd: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    deprecated_switches(cmd).iter().filter(|(d, _, _)| args.iter().any(|a| a == d)).copied().collect()
}

/// 죽은 스위치를 대체로 바꾼다(대체가 비어 있으면 그냥 뺀다). 순서는 유지한다.
pub(crate) fn replace_stale(args: &mut Vec<String>, dead: &str, fix: &str) {
    let Some(i) = args.iter().position(|a| a == dead) else { return };
    let repl: Vec<String> = fix.split_whitespace().map(str::to_string).collect();
    args.splice(i..i + 1, repl);
}

/// 프리셋 스위치 이름 포함 여부(설명 키 무시) — extra/preset 분리 판정용.
fn is_preset(cmd: &str, arg: &str) -> bool {
    preset_switches(cmd).iter().any(|(sw, _)| *sw == arg)
}

/// 설정 UI의 CLI 종류 선택지(마지막 "custom"은 자유 입력).
pub(crate) const CLI_CHOICES: [&str; 6] =
    ["claude", "codex", "antigravity", "aider", "gemini", "custom"];

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

    /// 프리셋 표가 스스로 모순되지 않는지 — 오타·중복은 조용히 깨진 명령줄이 된다.
    #[test]
    fn every_preset_switch_is_well_formed_and_unique() {
        for cli in CLI_CHOICES {
            let sw = preset_switches(cli);
            let mut seen = std::collections::HashSet::new();
            for (flag, key) in sw {
                assert!(flag.starts_with('-'), "{cli}: {flag} 은 옵션 모양이 아니다");
                assert!(!flag.contains(' '), "{cli}: {flag} 에 공백 — 값 있는 옵션은 '추가 인자'로");
                assert!(seen.insert(*flag), "{cli}: {flag} 이 두 번 들어 있다");
                assert_ne!(nabi_i18n::tr(nabi_i18n::Lang::Ko, key), *key, "{cli}: {key} 번역 없음");
            }
        }
    }

    /// 없어진 옵션은 프리셋에 남아 있으면 안 되고, 대체는 실제로 갈아 끼워져야 한다.
    #[test]
    fn a_dead_switch_is_never_offered_and_can_be_swapped() {
        for cli in CLI_CHOICES {
            for (dead, _, _) in deprecated_switches(cli) {
                assert!(
                    !preset_switches(cli).iter().any(|(f, _)| f == dead),
                    "{cli}: {dead} 은 없어졌는데 아직 체크박스로 제공된다"
                );
            }
        }
        let mut args = vec!["--search".to_string(), "--full-auto".into(), "-m".into(), "o3".into()];
        assert_eq!(stale_in(&args, "codex").len(), 1);
        replace_stale(&mut args, "--full-auto", "--sandbox workspace-write");
        assert_eq!(args, ["--search", "--sandbox", "workspace-write", "-m", "o3"]);
        assert!(stale_in(&args, "codex").is_empty());
    }

    /// codex 0.149.1에는 --full-auto 가 없다(직접 확인) — 되살아나면 실행이 통째로 실패한다.
    #[test]
    fn codex_no_longer_offers_full_auto() {
        assert!(!preset_switches("codex").iter().any(|(f, _)| *f == "--full-auto"));
        let bypass = "--dangerously-bypass-approvals-and-sandbox";
        assert!(preset_switches("codex").iter().any(|(f, _)| *f == bypass));
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
