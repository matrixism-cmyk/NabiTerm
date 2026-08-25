//! 자동 응답 실행 — 화면 끝을 훑어 규칙에 맞으면 답을 보낸다.
//!
//! 판단은 전부 `autoreply`의 순수 함수가 한다. 여기서는 화면을 읽어 넘기고, 결과를 보내고,
//! 연속 발동을 센다.
//!
//! ## 왜 같은 화면에 두 번 답하지 않는가
//!
//! 화면은 프레임마다 그대로 있다. 프롬프트가 떠 있는 동안 계속 답하면 한 번 물었는데
//! 수십 번 답하게 된다. 그래서 **화면 끝이 달라졌을 때만** 다시 본다.

use crate::app::NabiApp;
use crate::autoreply::{decide, Blocked, TAIL_ROWS};
use nabi_types::PaneId;
use std::time::{Duration, Instant};

/// 얼마나 자주 볼 것인가. 프롬프트는 사람이 읽고 답하는 것이라 이 정도면 충분히 빠르다.
const EVERY: Duration = Duration::from_millis(700);

impl NabiApp {
    /// 켜져 있으면 pane들의 화면 끝을 보고 정해 둔 답을 보낸다.
    pub(crate) fn check_auto_reply(&mut self) {
        if !self.config.terminal.auto_reply {
            return;
        }
        if self.auto_reply_check.elapsed() < EVERY {
            return;
        }
        self.auto_reply_check = Instant::now();
        let rules: Vec<(String, crate::triggers::Action)> = self
            .config
            .terminal
            .alert_patterns
            .iter()
            .filter_map(|p| crate::triggers::parse_rule(p))
            .collect();
        if !rules.iter().any(|(_, a)| matches!(a, crate::triggers::Action::Reply(_))) {
            return; // 답할 규칙이 하나도 없으면 화면을 읽지도 않는다.
        }
        // 화면 끝을 모은다(가드는 짧게 — 여기서 오래 잡으면 렌더가 멈춘다).
        let mut tails: Vec<(PaneId, String)> = Vec::new();
        if let Ok(panes) = self.orch.panes.read() {
            for (id, v) in panes.iter() {
                if let Ok(md) = v.model.lock() {
                    tails.push((*id, md.visible_bottom_text(TAIL_ROWS)));
                }
            }
        }
        for (pane, tail) in tails {
            self.reply_to_pane(pane, &tail, &rules);
        }
    }

    /// pane 하나를 판단하고 필요하면 답을 보낸다.
    fn reply_to_pane(&mut self, pane: PaneId, tail: &str, rules: &[(String, crate::triggers::Action)]) {
        // 같은 화면에 두 번 답하지 않는다.
        if self.auto_reply_seen.get(&pane).map(|s| s.as_str()) == Some(tail) {
            return;
        }
        let streak = self.auto_reply_streak.get(&pane).copied();
        match decide(tail, rules, streak) {
            Ok(None) => {
                self.auto_reply_seen.insert(pane, tail.to_string());
            }
            Ok(Some((idx, text))) => {
                self.orch.send(nabi_proto::Command::WriteInput { pane, data: text.clone().into() });
                self.auto_reply_seen.insert(pane, tail.to_string());
                let next = match streak {
                    Some((n, c)) if n == idx => c + 1,
                    _ => 1,
                };
                self.auto_reply_streak.insert(pane, (idx, next));
                let shown = text.trim_end_matches('\r');
                self.notify = Some((format!("\u{21b5} {shown}"), Instant::now()));
            }
            Err(why) => {
                // 막았으면 **말한다.** 조용히 안 하면 사용자는 규칙이 틀린 줄 안다.
                self.auto_reply_seen.insert(pane, tail.to_string());
                let key = match why {
                    Blocked::LooksLikeSecret => "autoreply.blocked.secret",
                    Blocked::TooManyTimes => "autoreply.blocked.streak",
                    Blocked::NonAscii => "autoreply.blocked.ascii",
                };
                // 비밀번호 프롬프트는 늘 뜨는 것이라 매번 알리면 시끄럽다 — 한 번만.
                if !matches!(why, Blocked::LooksLikeSecret) || self.auto_reply_streak.remove(&pane).is_some() {
                    self.notify = Some((nabi_i18n::tr(self.lang, key).to_string(), Instant::now()));
                }
            }
        }
    }

    /// pane이 사라지면 기억도 지운다.
    pub(crate) fn forget_auto_reply(&mut self, pane: PaneId) {
        self.auto_reply_seen.remove(&pane);
        self.auto_reply_streak.remove(&pane);
    }
}
