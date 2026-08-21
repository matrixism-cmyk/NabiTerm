//! AI 명령 바의 명령 표 — CLI별 슬래시 명령·표시 라벨·설명(UI는 aicmdbar.rs).
//!
//! 버튼에는 **한눈에 읽히는 요약명**(대화 요약·새 대화…)을 보여주고, 실제 슬래시 명령은
//! 툴팁에 설명과 함께 붙인다(사용자 요청 2026-08-19 — CLI를 GUI처럼).
//! 명령 목록은 각 CLI의 2026-08 공식 문서로 검증(claude commands·codex slash-commands·
//! Antigravity CLI(agy) 슬래시 명령·aider in-chat commands).

/// 바에 노출할 명령 하나.
pub(crate) struct BarCmd {
    /// 전송할 슬래시 명령("/compact").
    pub cmd: &'static str,
    /// 버튼에 보일 짧은 요약명 i18n 키.
    pub label: &'static str,
    /// 툴팁 설명 i18n 키.
    pub desc: &'static str,
    /// 하위 선택지[(표시, 전송 명령)] — 비면 클릭 즉시 `cmd` 전송.
    pub sub: &'static [(&'static str, &'static str)],
    /// 이 명령이 CLI 안에 **화면/선택창을 여는가**. true면 바 버튼이 활성색으로 바뀌고,
    /// 다시 누르면 ESC를 보내 닫는다(사용자 요청 2026-08-19). 즉시 끝나는 작업은 false.
    pub opens_ui: bool,
}

/// 즉시 끝나는 명령(대화 요약·새 대화 등).
const fn c(cmd: &'static str, label: &'static str, desc: &'static str) -> BarCmd {
    BarCmd { cmd, label, desc, sub: &[], opens_ui: false }
}

/// 화면/선택창을 여는 명령(재클릭 시 ESC로 닫는다).
const fn u(cmd: &'static str, label: &'static str, desc: &'static str) -> BarCmd {
    BarCmd { cmd, label, desc, sub: &[], opens_ui: true }
}

/// 실행 명령 → 명령 바를 아는 CLI 종류(판정은 aihandoff::ai_command_name과 공유 — SSOT).
pub(crate) fn bar_kind(run_cmd: &str) -> Option<&'static str> {
    crate::aihandoff::ai_command_name(run_cmd)
        .filter(|n| matches!(*n, "claude" | "codex" | "agy" | "aider"))
}

/// 주요 명령(바에 바로 노출). 나머지는 "⋯" 더보기 메뉴(secondary_commands).
pub(crate) fn primary_commands(kind: &str) -> &'static [BarCmd] {
    match kind {
        "claude" => {
            static A: &[BarCmd] = &[
                c("/compact", "aicb.l.compact", "aicb.claude.compact"),
                c("/clear", "aicb.l.clear", "aicb.claude.clear"),
                u("/context", "aicb.l.context", "aicb.claude.context"),
                // 별칭·단계는 공식 CLI 레퍼런스 기준(fable/opus/sonnet/haiku, low~ultracode).
                // 첫 항목은 CLI의 대화식 선택창.
                BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.claude.model", sub: &[
                    ("/model", "/model"), ("fable", "/model fable"), ("opus", "/model opus"),
                    ("sonnet", "/model sonnet"), ("haiku", "/model haiku"),
                ], opens_ui: true },
                BarCmd { cmd: "/effort", label: "aicb.l.effort", desc: "aicb.claude.effort", sub: &[
                    ("/effort", "/effort"), ("low", "/effort low"), ("medium", "/effort medium"),
                    ("high", "/effort high"), ("xhigh", "/effort xhigh"), ("max", "/effort max"),
                    ("ultracode", "/effort ultracode"),
                ], opens_ui: true },
                u("/resume", "aicb.l.resume", "aicb.claude.resume"),
                u("/usage", "aicb.l.usage", "aicb.claude.usage"),
            ];
            A
        }
        "codex" => {
            static A: &[BarCmd] = &[
                c("/compact", "aicb.l.compact", "aicb.codex.compact"),
                c("/clear", "aicb.l.clear", "aicb.codex.clear"),
                u("/permissions", "aicb.l.permissions", "aicb.codex.permissions"),
                u("/diff", "aicb.l.diff", "aicb.codex.diff"),
                BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.codex.model", sub: &[], opens_ui: true },
                u("/status", "aicb.l.status", "aicb.codex.status"),
            ];
            A
        }
        // Antigravity CLI(`agy`) — 공식 레퍼런스 antigravity.google/docs/cli/reference 기준.
        // Gemini CLI(2026-06-18 종료)의 /compress·/stats·/tools·/chat 등은 더 이상 없다.
        "agy" => {
            static A: &[BarCmd] = &[
                c("/clear", "aicb.l.clear", "aicb.agy.clear"),
                u("/context", "aicb.l.context", "aicb.agy.context"),
                u("/usage", "aicb.l.usage", "aicb.agy.usage"),
                u("/model", "aicb.l.model", "aicb.agy.model"),
                u("/resume", "aicb.l.resume", "aicb.agy.resume"),
                u("/diff", "aicb.l.diff", "aicb.agy.diff"),
            ];
            A
        }
        _ => {
            static A: &[BarCmd] = &[
                c("/undo", "aicb.l.undo", "aicb.aider.undo"),
                u("/diff", "aicb.l.diff", "aicb.aider.diff"),
                c("/commit", "aicb.l.commit", "aicb.aider.commit"),
                c("/clear", "aicb.l.clear", "aicb.aider.clear"),
                c("/tokens", "aicb.l.tokens", "aicb.aider.tokens"),
                c("/map", "aicb.l.map", "aicb.aider.map"),
            ];
            A
        }
    }
}

/// 더보기(⋯) 메뉴 명령.
pub(crate) fn secondary_commands(kind: &str) -> &'static [BarCmd] {
    match kind {
        "claude" => {
            static A: &[BarCmd] = &[
                u("/permissions", "aicb.l.permissions", "aicb.claude.permissions"),
                u("/mcp", "aicb.l.mcp", "aicb.claude.mcp"),
                u("/memory", "aicb.l.memory", "aicb.claude.memory"),
                u("/plan", "aicb.l.plan", "aicb.claude.plan"),
                u("/rewind", "aicb.l.rewind", "aicb.claude.rewind"),
                u("/diff", "aicb.l.diff", "aicb.claude.diff"),
                c("/init", "aicb.l.init", "aicb.claude.init"),
                u("/doctor", "aicb.l.doctor", "aicb.claude.doctor"),
                u("/export", "aicb.l.export", "aicb.claude.export"),
                u("/help", "aicb.l.help", "aicb.help"),
            ];
            A
        }
        "codex" => {
            static A: &[BarCmd] = &[
                u("/approve", "aicb.l.approve", "aicb.codex.approve"),
                u("/memories", "aicb.l.memory", "aicb.codex.memories"),
                u("/skills", "aicb.l.skills", "aicb.codex.skills"),
                u("/ide", "aicb.l.ide", "aicb.codex.ide"),
                c("/copy", "aicb.l.copy", "aicb.codex.copy"),
                u("/rename", "aicb.l.rename", "aicb.codex.rename"),
                c("/init", "aicb.l.init", "aicb.codex.init"),
            ];
            A
        }
        "agy" => {
            static A: &[BarCmd] = &[
                u("/agents", "aicb.l.agents", "aicb.agy.agents"),
                u("/permissions", "aicb.l.perms", "aicb.agy.perms"),
                u("/skills", "aicb.l.skills", "aicb.agy.skills"),
                u("/mcp", "aicb.l.mcp", "aicb.agy.mcp"),
                u("/tasks", "aicb.l.tasks", "aicb.agy.tasks"),
                c("/rewind", "aicb.l.rewind", "aicb.agy.rewind"),
                c("/copy", "aicb.l.copy", "aicb.agy.copy"),
                u("/config", "aicb.l.settings", "aicb.agy.config"),
                u("/help", "aicb.l.help", "aicb.help"),
            ];
            A
        }
        _ => {
            static A: &[BarCmd] = &[
                c("/drop", "aicb.l.drop", "aicb.aider.drop"),
                BarCmd { cmd: "/model", label: "aicb.l.model", desc: "aicb.aider.model", sub: &[], opens_ui: true },
                c("/test", "aicb.l.test", "aicb.aider.test"),
                c("/lint", "aicb.l.lint", "aicb.aider.lint"),
                u("/settings", "aicb.l.settings", "aicb.aider.settings"),
                u("/help", "aicb.l.help", "aicb.help"),
            ];
            A
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_strict_and_commands_ascii() {
        assert_eq!(bar_kind("claude --continue"), Some("claude"));
        assert_eq!(bar_kind(r"C:\bin\codex.exe"), Some("codex"));
        assert_eq!(bar_kind("grep claude src"), None, "부분문자열 오탐 금지");
        assert_eq!(bar_kind(""), None);
        for kind in ["claude", "codex", "agy", "aider"] {
            for bc in primary_commands(kind).iter().chain(secondary_commands(kind)) {
                assert!(bc.cmd.starts_with('/') && bc.cmd.is_ascii());
                assert!(bc.label.starts_with("aicb.l."), "표시 라벨 키 규약: {}", bc.label);
                for (_, cmd) in bc.sub {
                    assert!(cmd.is_ascii(), "주입 명령은 ASCII 전용: {cmd}");
                }
            }
        }
    }
}
