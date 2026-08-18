//! AI 명령 바 — AI CLI가 실행 중인 pane 상단에 그 CLI의 슬래시 명령을 클릭형으로 노출.
//!
//! "/를 눌러야 나오는 명령을 처음부터 위에" (사용자 요청 2026-08-18) — CLI를 GUI처럼.
//! 명령 목록은 각 CLI의 2026-08 공식 문서로 검증(claude commands·codex slash-commands·
//! gemini cli/commands). 클릭 = 해당 pane에 "/cmd\r" 주입(ASCII 전용 규칙 준수).

/// 바에 노출할 명령 하나: (표시 명령, 설명 i18n 키, 하위 선택지[(라벨, 전송 명령)]).
/// 하위 선택지가 비면 클릭 즉시 표시 명령을 그대로 보낸다.
pub(crate) struct BarCmd {
    pub cmd: &'static str,
    pub desc: &'static str,
    pub sub: &'static [(&'static str, &'static str)],
}

const fn c(cmd: &'static str, desc: &'static str) -> BarCmd {
    BarCmd { cmd, desc, sub: &[] }
}

/// 실행 명령 → 명령 바를 아는 CLI 종류(판정은 aihandoff::ai_command_name과 공유 — SSOT).
pub(crate) fn bar_kind(run_cmd: &str) -> Option<&'static str> {
    crate::aihandoff::ai_command_name(run_cmd)
        .filter(|n| matches!(*n, "claude" | "codex" | "gemini" | "aider"))
}

/// 주요 명령(바에 바로 노출). 나머지는 "⋯" 더보기 메뉴(secondary_commands).
pub(crate) fn primary_commands(kind: &str) -> &'static [BarCmd] {
    match kind {
        "claude" => { static A: &[BarCmd] = &[
            c("/compact", "aicb.claude.compact"),
            c("/clear", "aicb.claude.clear"),
            c("/context", "aicb.claude.context"),
            // 별칭·단계는 공식 CLI 레퍼런스 기준(2026-08: fable/opus/sonnet/haiku,
            // effort low~ultracode). 첫 항목은 대화식 선택창.
            BarCmd { cmd: "/model", desc: "aicb.claude.model", sub: &[
                ("/model", "/model"), ("fable", "/model fable"), ("opus", "/model opus"),
                ("sonnet", "/model sonnet"), ("haiku", "/model haiku"),
            ] },
            BarCmd { cmd: "/effort", desc: "aicb.claude.effort", sub: &[
                ("/effort", "/effort"), ("low", "/effort low"), ("medium", "/effort medium"),
                ("high", "/effort high"), ("xhigh", "/effort xhigh"), ("max", "/effort max"),
                ("ultracode", "/effort ultracode"),
            ] },
            c("/resume", "aicb.claude.resume"),
            c("/usage", "aicb.claude.usage"),
        ]; A }
        "codex" => { static A: &[BarCmd] = &[
            c("/compact", "aicb.codex.compact"),
            c("/clear", "aicb.codex.clear"),
            c("/permissions", "aicb.codex.permissions"),
            c("/diff", "aicb.codex.diff"),
            c("/model", "aicb.codex.model"),
            c("/status", "aicb.codex.status"),
        ]; A }
        "gemini" => { static A: &[BarCmd] = &[
            c("/compress", "aicb.gemini.compress"),
            c("/clear", "aicb.gemini.clear"),
            c("/stats", "aicb.gemini.stats"),
            c("/tools", "aicb.gemini.tools"),
            BarCmd { cmd: "/memory", desc: "aicb.gemini.memory", sub: &[
                ("show", "/memory show"), ("refresh", "/memory refresh"),
            ] },
            BarCmd { cmd: "/chat", desc: "aicb.gemini.chat", sub: &[
                ("list", "/chat list"), ("save", "/chat save"), ("resume", "/chat resume"),
            ] },
        ]; A }
        _ => { static A: &[BarCmd] = &[
            c("/undo", "aicb.aider.undo"),
            c("/diff", "aicb.aider.diff"),
            c("/commit", "aicb.aider.commit"),
            c("/clear", "aicb.aider.clear"),
            c("/tokens", "aicb.aider.tokens"),
            c("/map", "aicb.aider.map"),
        ]; A }
    }
}

/// 더보기(⋯) 메뉴 명령.
pub(crate) fn secondary_commands(kind: &str) -> &'static [BarCmd] {
    match kind {
        "claude" => { static A: &[BarCmd] = &[
            c("/permissions", "aicb.claude.permissions"),
            c("/mcp", "aicb.claude.mcp"),
            c("/memory", "aicb.claude.memory"),
            c("/plan", "aicb.claude.plan"),
            c("/rewind", "aicb.claude.rewind"),
            c("/diff", "aicb.claude.diff"),
            c("/init", "aicb.claude.init"),
            c("/doctor", "aicb.claude.doctor"),
            c("/export", "aicb.claude.export"),
            c("/help", "aicb.help"),
        ]; A }
        "codex" => { static A: &[BarCmd] = &[
            c("/approve", "aicb.codex.approve"),
            c("/memories", "aicb.codex.memories"),
            c("/skills", "aicb.codex.skills"),
            c("/ide", "aicb.codex.ide"),
            c("/copy", "aicb.codex.copy"),
            c("/rename", "aicb.codex.rename"),
            c("/init", "aicb.codex.init"),
        ]; A }
        "gemini" => { static A: &[BarCmd] = &[
            c("/mcp", "aicb.gemini.mcp"),
            c("/restore", "aicb.gemini.restore"),
            c("/init", "aicb.gemini.init"),
            c("/settings", "aicb.gemini.settings"),
            c("/extensions", "aicb.gemini.extensions"),
            c("/copy", "aicb.gemini.copy"),
            c("/help", "aicb.help"),
        ]; A }
        _ => { static A: &[BarCmd] = &[
            c("/drop", "aicb.aider.drop"),
            c("/model", "aicb.aider.model"),
            c("/test", "aicb.aider.test"),
            c("/lint", "aicb.aider.lint"),
            c("/settings", "aicb.aider.settings"),
            c("/help", "aicb.help"),
        ]; A }
    }
}

impl crate::tabs::TermTabViewer<'_> {
    /// pane에 AI 명령 바를 그린다(설정 꺼짐/비AI pane이면 아무것도 안 그림).
    /// 클릭된 명령은 "/cmd\r"로 그 pane에 주입(tabsterm에서 호출 — 라인 한도로 분리).
    pub(crate) fn ai_bar(&mut self, ui: &mut egui::Ui, pane: nabi_types::PaneId) {
        if !self.ai_cmd_bar {
            return;
        }
        let Some(kind) = self.run_cmd.get(&pane).and_then(|c| bar_kind(c)) else { return };
        if let Some(cmd) = show_bar(ui, self.lang, kind) {
            let mut data = cmd.into_bytes();
            data.push(b'\r');
            self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(data) });
        }
    }
}

/// 명령 바 UI. 반환 = 주입할 명령(선택 시). `kind`는 bar_kind 결과.
pub(crate) fn show_bar(ui: &mut egui::Ui, lang: nabi_i18n::Lang, kind: &'static str) -> Option<String> {
    use nabi_i18n::tr;
    let mut send = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.small(egui::RichText::new(format!("🤖 {kind}")).color(crate::theme_ui::ACCENT));
        for bc in primary_commands(kind) {
            if bc.sub.is_empty() {
                if ui.small_button(bc.cmd).on_hover_text(tr(lang, bc.desc)).clicked() {
                    send = Some(bc.cmd.to_string());
                }
            } else {
                ui.menu_button(format!("{} ▾", bc.cmd), |ui| {
                    for (label, cmd) in bc.sub {
                        if ui.button(*label).clicked() {
                            send = Some((*cmd).to_string());
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tr(lang, bc.desc));
            }
        }
        ui.menu_button("⋯", |ui| {
            for bc in secondary_commands(kind) {
                if ui.button(bc.cmd).on_hover_text(tr(lang, bc.desc)).clicked() {
                    send = Some(bc.cmd.to_string());
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(tr(lang, "aicb.more"));
    });
    send
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
        for kind in ["claude", "codex", "gemini", "aider"] {
            for bc in primary_commands(kind).iter().chain(secondary_commands(kind)) {
                assert!(bc.cmd.starts_with('/') && bc.cmd.is_ascii());
                for (_, cmd) in bc.sub {
                    assert!(cmd.is_ascii(), "주입 명령은 ASCII 전용: {cmd}");
                }
            }
        }
    }
}
