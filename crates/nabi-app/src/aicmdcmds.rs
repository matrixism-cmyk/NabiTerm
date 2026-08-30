//! AI 명령 바의 명령 표 — CLI별 슬래시 명령·표시 라벨·설명(UI는 aicmdbar.rs).
//!
//! 버튼에는 **한눈에 읽히는 요약명**(대화 요약·새 대화…)을 보여주고, 실제 슬래시 명령은
//! 툴팁에 설명과 함께 붙인다(사용자 요청 2026-08-19 — CLI를 GUI처럼).
//! 명령 목록은 각 CLI의 2026-08 공식 문서로 검증(claude commands·codex slash-commands·
//! Antigravity CLI(agy) 슬래시 명령·aider in-chat commands).
//!
//! Claude는 명령이 80개를 넘어 표를 aicmdclaude.rs로 분리하고 **주제별 묶음**으로 낸다.

/// 바에 노출할 명령 하나.
pub(crate) struct BarCmd {
    /// 전송할 슬래시 명령("/compact").
    pub cmd: &'static str,
    /// 버튼·메뉴에 보일 짧은 요약명 i18n 키. 비어 있으면 `cmd`를 그대로 보여준다(마지막 방어선 —
    /// 정상 경로에서는 모든 명령이 요약명을 가진다. 테스트가 그것을 강제한다).
    pub label: &'static str,
    /// 툴팁 설명 i18n 키.
    pub desc: &'static str,
    /// 하위 선택지[(표시, 전송 명령)] — 비면 클릭 즉시 `cmd` 전송.
    pub sub: &'static [(&'static str, &'static str)],
    /// 이 명령이 CLI 안에 **화면/선택창을 여는가**. true면 바 버튼이 활성색으로 바뀌고,
    /// 다시 누르면 ESC를 보내 닫는다(사용자 요청 2026-08-19). 즉시 끝나는 작업은 false.
    pub opens_ui: bool,
}

/// 더보기 메뉴의 한 묶음. `label`이 비면 하위 메뉴 없이 그대로 펼친다.
pub(crate) struct CmdGroup {
    pub label: &'static str,
    pub cmds: &'static [BarCmd],
}

/// 즉시 끝나는 명령(대화 요약·새 대화 등).
pub(crate) const fn c(cmd: &'static str, label: &'static str, desc: &'static str) -> BarCmd {
    BarCmd { cmd, label, desc, sub: &[], opens_ui: false }
}

/// 화면/선택창을 여는 명령(재클릭 시 ESC로 닫는다).
pub(crate) const fn u(cmd: &'static str, label: &'static str, desc: &'static str) -> BarCmd {
    BarCmd { cmd, label, desc, sub: &[], opens_ui: true }
}

/// 실행 명령 → 명령 바를 아는 CLI 종류(판정은 aihandoff::ai_command_name과 공유 — SSOT).
pub(crate) fn bar_kind(run_cmd: &str) -> Option<&'static str> {
    crate::aihandoff::ai_command_name(run_cmd)
        .filter(|n| matches!(*n, "claude" | "codex" | "agy" | "aider" | "gemini"))
}

/// 주요 명령(바에 바로 노출). 나머지는 "⋯" 더보기 메뉴(secondary_groups).
pub(crate) fn primary_commands(kind: &str) -> &'static [BarCmd] {
    match kind {
        "claude" => crate::aicmdclaude::primary(),
        "codex" => crate::aicmdother::codex_primary(),
        "agy" => crate::aicmdother::agy_primary(),
        "gemini" => crate::aicmdgemini::primary(),
        _ => crate::aicmdother::aider_primary(),
    }
}

/// 더보기(⋯) 메뉴 — 주제별 묶음.
pub(crate) fn secondary_groups(kind: &str) -> &'static [CmdGroup] {
    match kind {
        "claude" => crate::aicmdclaude::groups(),
        "codex" => crate::aicmdother::codex_groups(),
        "agy" => crate::aicmdother::agy_groups(),
        "gemini" => crate::aicmdgemini::groups(),
        _ => crate::aicmdother::aider_groups(),
    }
}

/// 이 CLI를 끝내는 명령. 바 오른쪽 "⋯" **바로 앞**에 종료 버튼으로 노출한다
/// (사용자 요청 2026-08-22 — 끝내려고 매번 더보기를 열지 않도록).
///
/// 네 CLI가 모두 `/exit`을 받는다(claude·codex·agy·aider 공식 문서에서 확인).
/// 달라지는 CLI가 생기면 여기만 고치면 된다 — 버튼은 이 값을 그대로 보낸다.
pub(crate) const QUIT_CMD: &str = "/exit";

/// 더보기 메뉴의 모든 명령(묶음을 펼친 것) — 검색 필터·테스트가 쓴다.
pub(crate) fn secondary_flat(kind: &str) -> impl Iterator<Item = &'static BarCmd> {
    secondary_groups(kind).iter().flat_map(|g| g.cmds.iter())
}

/// 명령 바가 아는 CLI 종류 — **한 곳에서만 적는다.**
///
/// 시험 일곱 군데가 이 목록을 저마다 손으로 들고 있었다. 새 CLI 를 붙이면 일곱 곳을 다
/// 고쳐야 하는데, 하나만 빠뜨려도 그 CLI 는 검사 없이 지나간다 — 실제로 gemini 를 붙이며
/// 드러났다. `bar_kind` 가 거르는 목록과 여기가 같아야 하고, 아래 시험이 그것을 지킨다.
#[cfg(test)]
pub(crate) const KINDS: [&str; 5] = ["claude", "codex", "agy", "aider", "gemini"];

#[cfg(test)]
mod tests {
    use super::*;

    /// `KINDS` 와 `bar_kind` 가 거르는 목록이 같은가.
    ///
    /// 소스를 글자로 대조한다 — 한쪽에만 CLI 를 더하면 여기서 걸린다.
    #[test]
    fn 아는_종류_목록이_한곳과_같다() {
        let src = include_str!("aicmdcmds.rs");
        let at = src.find("matches!(*n,").expect("bar_kind 의 목록을 못 찾았다");
        let line: String = src[at..].chars().take_while(|c| *c != ')').collect();
        for k in KINDS {
            assert!(line.contains(&format!("\"{k}\"")), "bar_kind 가 {k} 를 모른다");
        }
        // 반대로 bar_kind 에만 있는 것도 없어야 한다.
        let n = line.matches('"').count() / 2;
        assert_eq!(n, KINDS.len(), "목록 개수가 다르다: bar_kind {n} · KINDS {}", KINDS.len());
    }

    #[test]
    fn kinds_are_strict_and_commands_ascii() {
        assert_eq!(bar_kind("claude --continue"), Some("claude"));
        assert_eq!(bar_kind(r"C:\bin\codex.exe"), Some("codex"));
        assert_eq!(bar_kind("grep claude src"), None, "부분문자열 오탐 금지");
        assert_eq!(bar_kind(""), None);
        for kind in KINDS {
            for bc in primary_commands(kind).iter().chain(secondary_flat(kind)) {
                assert!(bc.cmd.starts_with('/') && bc.cmd.is_ascii());
                assert!(bc.desc.starts_with("aicb."), "설명 키 규약: {}", bc.desc);
                for (_, cmd) in bc.sub {
                    assert!(cmd.is_ascii(), "주입 명령은 ASCII 전용: {cmd}");
                }
            }
            // 바 버튼이든 더보기든 **모두 요약명을 가진다** — 표기 규칙이 갈라지면
            // 새로 넣은 명령만 `/cmd`로 나와 사용자가 다른 물건으로 읽는다(2026-08-22 지적).
            for bc in primary_commands(kind).iter().chain(secondary_flat(kind)) {
                assert!(bc.label.starts_with("aicb.l."), "표시 라벨 키 규약: {} ({})", bc.label, bc.cmd);
            }
        }
    }

    /// 라벨·설명 키가 실제로 카탈로그에 있어야 한다. `tr`은 없는 키에 `"?"`를 돌려주므로
    /// 빠뜨리면 버튼에 물음표가 뜬다 — 화면을 열어 보기 전에는 아무도 모른다.
    #[test]
    fn every_key_is_translated_in_all_languages() {
        use nabi_i18n::{tr, Lang};
        for kind in KINDS {
            for bc in primary_commands(kind).iter().chain(secondary_flat(kind)) {
                for lang in [Lang::En, Lang::Ko, Lang::Ja] {
                    assert_ne!(tr(lang, bc.label), "?", "{lang:?} 라벨 없음: {}", bc.label);
                    assert_ne!(tr(lang, bc.desc), "?", "{lang:?} 설명 없음: {}", bc.desc);
                }
            }
            for g in secondary_groups(kind).iter().filter(|g| !g.label.is_empty()) {
                assert_ne!(tr(Lang::Ko, g.label), "?", "묶음 이름 없음: {}", g.label);
            }
        }
    }

    /// 같은 명령이 바와 더보기에 **동시에** 나오면 사용자가 헷갈린다(드리프트 방지).
    #[test]
    fn no_duplicate_between_primary_and_more() {
        for kind in KINDS {
            let prim: Vec<_> = primary_commands(kind).iter().map(|b| b.cmd).collect();
            for bc in secondary_flat(kind) {
                assert!(!prim.contains(&bc.cmd), "{kind}: {} 중복", bc.cmd);
            }
        }
    }

    /// 종료는 바에 **버튼으로** 나온다 — 메뉴에도 있으면 같은 일이 두 군데가 된다.
    #[test]
    fn quit_lives_only_on_the_button() {
        for kind in KINDS {
            for bc in primary_commands(kind).iter().chain(secondary_flat(kind)) {
                assert_ne!(bc.cmd, QUIT_CMD, "{kind}: 종료가 목록에도 있다");
            }
        }
    }

    /// 각 CLI가 실제로 그만큼의 명령을 갖췄는가 — 예전처럼 몇 개만 남아 빈약해지지 않게.
    #[test]
    fn every_cli_has_a_useful_number_of_commands() {
        for (kind, least) in [("claude", 70), ("codex", 30), ("agy", 25), ("aider", 30)] {
            let n = primary_commands(kind).len() + secondary_flat(kind).count();
            assert!(n >= least, "{kind}: {n}개뿐 — 최소 {least}개는 되어야 한다");
        }
    }

    /// 더보기 안에서도 중복이 없어야 한다(묶음을 나누다 실수하기 쉬운 자리).
    #[test]
    fn more_menu_has_no_repeats() {
        for kind in KINDS {
            let mut seen = std::collections::BTreeSet::new();
            for bc in secondary_flat(kind) {
                assert!(seen.insert(bc.cmd), "{kind}: {} 두 번", bc.cmd);
            }
        }
    }
}
