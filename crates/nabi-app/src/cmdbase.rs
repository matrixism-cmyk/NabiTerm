//! **명령줄에서 실제로 도는 프로그램 이름**을 뽑는다 — 껍데기를 벗기고, 거기서 멈춘다.
//!
//! ## 왜 따로 있는가
//!
//! 두 곳이 같은 질문을 각자 답하고 있었다. 상태바 배지(`aistatus`)와 인계·명령 바
//! (`aihandoff`)가 각각 "첫 토막을 잘라 목록과 견주는" 코드를 갖고 있었고, 둘 다
//! **감싸서 띄운 것을 못 알아봤다.** 같은 결함을 두 번 고치지 않으려고 여기로 모았다.
//!
//! ## 어디까지 벗기나
//!
//! `npx claude`·`sudo codex`·`wsl agy`·`pwsh -c claude`·`env FOO=1 codex`·`cmd /c claude` —
//! 전부 흔한 방법인데 예전에는 하나도 안 걸렸다(2026-09-01 탐침으로 확인).
//! 껍데기와 플래그, `KEY=VALUE` 를 건너뛰고 **처음 만나는 진짜 명령**을 돌려준다.
//!
//! ## 왜 거기서 멈추나
//!
//! `sudo apt install claude` 를 끝까지 훑으면 `claude` 를 찾아낸다. 그런데 그 pane 에서
//! 도는 것은 `apt` 지 AI 가 아니다. 그걸 AI 로 보면 **남의 셸에 프롬프트가 찍힌다.**
//! 껍데기가 아닌 것을 처음 만나면 그것이 답이고, 맞든 아니든 거기서 끝이다.

/// 앞에 붙어 실제 명령을 감싸는 것들. **이것만** 건너뛴다.
const LAUNCHERS: [&str; 16] = [
    "sudo", "doas", "env", "npx", "bunx", "pnpm", "yarn", "uvx", "uv", "wsl", "cmd", "pwsh",
    "powershell", "bash", "nohup", "winpty",
];

/// **값을 하나 받는 플래그** — 그 뒤 토막은 명령이 아니라 값이다.
///
/// `sudo -u kim codex` 에서 `kim` 을 명령으로 보면 안 된다. 대개는 목록에 없는 이름이라
/// 조용히 넘어가지만, 계정 이름이 하필 `claude` 라면 **그 pane 을 AI 로 착각해 프롬프트를
/// 밀어 넣는다.** 드물어도 방향이 나쁜 실수라 막는다.
const FLAGS_WITH_VALUE: [&str; 8] =
    ["-u", "--user", "-p", "--package", "-g", "--group", "-w", "--workdir"];

/// 이 명령줄이 실제로 실행하는 프로그램의 이름(경로·`.exe` 제거, 소문자). 없으면 `None`.
pub(crate) fn real_command_base(cmd: &str) -> Option<String> {
    let mut skip_next = false;
    for tok in cmd.split_whitespace() {
        if std::mem::take(&mut skip_next) {
            continue; // 앞 플래그의 값.
        }
        // 플래그(`-c`·`--yes`)와 `cmd /c`·`/k` 스위치.
        if tok.starts_with('-') || matches!(tok, "/c" | "/k" | "/C" | "/K") {
            skip_next = FLAGS_WITH_VALUE.contains(&tok);
            continue;
        }
        let name = base_name(tok);
        // `FOO=1` 같은 환경변수 지정.
        if name.contains('=') {
            continue;
        }
        if LAUNCHERS.contains(&name.as_str()) {
            continue;
        }
        return Some(name);
    }
    None
}

/// 경로와 `.exe` 를 떼고 소문자로. `@scope/pkg` 도 마지막 조각만 본다.
fn base_name(tok: &str) -> String {
    tok.rsplit(['\\', '/']).next().unwrap_or(tok).trim_end_matches(".exe").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::real_command_base;

    fn b(s: &str) -> Option<String> {
        real_command_base(s)
    }

    #[test]
    fn a_plain_command_is_its_own_base() {
        assert_eq!(b("claude").as_deref(), Some("claude"));
        assert_eq!(b("codex --yolo").as_deref(), Some("codex"));
        assert_eq!(b(r"C:\bin\agy.exe").as_deref(), Some("agy"));
    }

    /// **감싸서 띄운 것도 벗겨 낸다** — 여섯 가지 전부 예전에는 안 걸렸다.
    #[test]
    fn wrappers_are_peeled_off() {
        for (cmd, want) in [
            ("npx claude", "claude"),
            ("sudo codex", "codex"),
            ("wsl agy", "agy"),
            ("pwsh -c claude", "claude"),
            ("uvx aider", "aider"),
            ("env FOO=1 codex", "codex"),
            ("cmd /c claude", "claude"),
            ("npx -y @anthropic-ai/claude-code", "claude-code"),
        ] {
            assert_eq!(b(cmd).as_deref(), Some(want), "{cmd:?}");
        }
    }

    /// **껍데기 뒤 첫 명령에서 멈춘다.** 뒤쪽에 이름이 섞였다고 따라가면 안 된다 —
    /// 그 pane 에서 도는 것은 `apt` 지 AI 가 아니다.
    #[test]
    fn it_stops_at_the_first_real_command() {
        assert_eq!(b("sudo apt install claude").as_deref(), Some("apt"));
        assert_eq!(b("npx eslint --fix claude.js").as_deref(), Some("eslint"));
    }

    /// 껍데기만 있으면 답이 없다.
    #[test]
    fn nothing_to_run_means_none() {
        assert_eq!(b(""), None);
        assert_eq!(b("sudo -u kim env"), None);
    }

    /// **플래그의 값을 명령으로 보면 안 된다.** 계정 이름이 하필 AI CLI 이름이면
    /// 그 pane 을 AI 로 착각해 프롬프트를 밀어 넣게 된다.
    #[test]
    fn a_flag_value_is_not_the_command() {
        assert_eq!(b("sudo -u kim codex").as_deref(), Some("codex"));
        assert_eq!(b("sudo -u claude apt update").as_deref(), Some("apt"), "계정 이름에 속으면 안 된다");
        assert_eq!(b("npx -p @scope/tool claude").as_deref(), Some("claude"));
    }
}
