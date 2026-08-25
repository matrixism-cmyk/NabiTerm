//! 출력 트리거 알림(MobaXterm trigger/Warp식) — 새 터미널 출력에 사용자 지정 패턴이 나타나면
//! 토스트 + (비포커스 시) 작업표시줄 주의 환기. 빌드 완료·에러 등을 다른 작업 중에도 인지(AI 코딩).
//! line_marker 델타로 신규 줄만 검사(세션 로깅과 같은 패턴).

use crate::app::NabiApp;
use nabi_types::PaneId;
use std::time::{Duration, Instant};

impl NabiApp {
    /// ~1초마다 모든 pane의 신규 출력 줄을 검사해 패턴 일치 시 알림. 패턴 없으면 즉시 반환.
    pub(crate) fn check_output_alerts(&mut self, ctx: &egui::Context) {
        if self.config.terminal.alert_patterns.iter().all(|p| p.trim().is_empty()) {
            return;
        }
        if self.alert_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.alert_check = Instant::now();
        // 자동 응답 규칙(-> reply:)은 **여기서 다루지 않는다.** 알림은 "새로 생긴 줄"을 보는데
        // 프롬프트는 줄바꿈 없이 커서 앞에 머물러 새 줄로 안 잡힌다. 그쪽은 화면 끝을 보는
        // 별도 경로(autoreply)가 맡는다 — 여기 섞으면 답도 안 되면서 토스트만 뜬다.
        let pats: Vec<(String, Action)> = self.config.terminal.alert_patterns.iter()
            .filter_map(|p| parse_rule(p))
            .filter(|(_, a)| !matches!(a, Action::Reply(_)))
            .collect();
        // (pane, 신규줄 텍스트) 수집(가드 분리).
        let mut hits: Vec<(PaneId, String, Action)> = Vec::new();
        if let Ok(panes) = self.orch.panes.read() {
            for (id, v) in panes.iter() {
                let Ok(md) = v.model.lock() else { continue };
                let cur = md.line_marker();
                let last = *self.alert_marks.get(id).unwrap_or(&cur); // 처음 보는 pane은 현재부터.
                if cur > last {
                    let text = md.lines_abs_text(last, cur).join("\n").to_lowercase();
                    if let Some((p, act)) = pats.iter().find(|(p, _)| text.contains(p.as_str())) {
                        hits.push((*id, p.clone(), act.clone()));
                    }
                }
                self.alert_marks.insert(*id, cur);
            }
        }
        // 액션(C4): 토스트는 항상(가시성), 접미 액션은 추가 실행.
        for (_, pat, act) in &hits {
            match act {
                Action::Toast => {}
                Action::Telegram => {
                    if let Some(owner) = self.telegram_owner() {
                        self.telegram.reply(owner, format!("\u{1f514} {pat}"));
                    }
                }
                // 위에서 걸러 냈으므로 여기 올 수 없다. 그래도 조용히 넘기지 않고 남긴다.
                Action::Reply(_) => {}
                Action::Command(cmd) => {
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("powershell")
                        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", cmd])
                        .creation_flags(0x0800_0000)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
        }
        if let Some((_, pat, _)) = hits.first() {
            self.notify = Some((format!("\u{1f514} {pat}"), Instant::now()));
            if !ctx.input(|i| i.focused) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
            }
        }
    }
}

/// 트리거 액션(C4): 접미 없음=토스트만(기존), `-> telegram`=오너 발신, `-> command:<셸>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Toast,
    Telegram,
    Command(String),
    /// **원격에 이 글자를 보낸다**(자동 응답). 안전장치는 `autoreply` 참고 —
    /// 알림과 달리 이것은 되돌릴 수 없는 결과를 낼 수 있어 별도 판정을 거친다.
    Reply(String),
}

/// 규칙 한 줄 파싱: `패턴 [-> 액션]`. 패턴은 소문자 비교(기존 동작 유지).
pub(crate) fn parse_rule(entry: &str) -> Option<(String, Action)> {
    let e = entry.trim();
    if e.is_empty() {
        return None;
    }
    let (pat, act) = match e.split_once("->") {
        Some((p, a)) => {
            let a = a.trim();
            let act = if a == "telegram" {
                Action::Telegram
            } else if let Some(c) = a.strip_prefix("command:") {
                Action::Command(c.trim().to_string())
            } else if let Some(r) = a.strip_prefix("reply:") {
                // 답의 앞뒤 공백은 뜻이 있을 수 있어(들여쓰기 응답) 끝의 공백만 남긴다.
                Action::Reply(r.trim_start().to_string())
            } else {
                Action::Toast // 모르는 액션은 토스트로 강등(조용한 무시 금지).
            };
            (p, act)
        }
        None => (e, Action::Toast),
    };
    let pat = pat.trim().to_lowercase();
    (!pat.is_empty()).then_some((pat, act))
}

#[cfg(test)]
mod tests {
    use super::{parse_rule, Action};

    #[test]
    fn parses_rule_suffixes() {
        assert_eq!(parse_rule("BUILD FAILED"), Some(("build failed".into(), Action::Toast)));
        assert_eq!(parse_rule("deploy done -> telegram"), Some(("deploy done".into(), Action::Telegram)));
        assert_eq!(
            parse_rule("tests passed -> command: git push"),
            Some(("tests passed".into(), Action::Command("git push".into())))
        );
        // 모르는 액션은 토스트 강등, 빈 패턴은 무시.
        assert_eq!(parse_rule("x -> shutdown").map(|r| r.1), Some(Action::Toast));
        assert_eq!(parse_rule("   "), None);
        assert_eq!(parse_rule(" -> telegram"), None);
    }
}

