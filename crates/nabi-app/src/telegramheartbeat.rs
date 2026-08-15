//! 텔레그램 메시지 라우팅(수신 처리·명령) + 하트비트(C5). telegrambridge.rs에서 분리(라인 한도).

use crate::app::NabiApp;
use nabi_telegram::TgMessage;
use nabi_types::PaneId;
use std::time::Instant;

impl NabiApp {
    /// 하트비트(C5): N분마다 에이전트 상태 요약을 오너에게 — **변화가 있을 때만**.
    /// OpenClaw의 HEARTBEAT_OK 억제 패턴: 조용한 하트비트는 보내지 않는 게 정보다.
    pub(crate) fn telegram_heartbeat_tick(&mut self) {
        let mins = self.config.telegram.heartbeat_mins;
        if mins == 0 || !self.telegram.running() {
            return;
        }
        let due = self.telegram_heartbeat.0
            .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(mins * 60));
        if !due {
            return;
        }
        self.telegram_heartbeat.0 = Some(Instant::now());
        let Some(owner) = self.telegram_owner() else { return };
        let summary = self.agent_summary();
        if summary == self.telegram_heartbeat.1 {
            return; // 변화 없음 — 무발신.
        }
        self.telegram_heartbeat.1 = summary.clone();
        self.telegram.reply(owner, format!("\u{1f4e1} {summary}"));
    }

    /// 에이전트 pane 상태 요약(제목: 상태 나열). 에이전트가 없으면 빈 요약.
    fn agent_summary(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        if let Ok(panes) = self.orch.panes.read() {
            let empty = std::collections::BTreeMap::new();
            for (p, v) in panes.iter() {
                let st = self.pane_status.get(p).unwrap_or(&empty);
                let watched = self.agent_watch.state.contains_key(p);
                if st.is_empty() && !watched {
                    continue; // 에이전트 아님.
                }
                let s = self.merged_agent_state(*p, st, self.cmd_start.contains_key(p));
                let name = match s { 2 => "blocked", 1 => "working", 3 => "done", _ => "idle" };
                lines.push(format!("{}: {name}", v.title));
            }
        }
        lines.sort();
        lines.join("\n")
    }

    /// 명령이 끝난 pane(OSC133 CommandBlock)을 기다리던 보류에 실제 출력을 회신한다(events.rs에서 호출).
    pub(crate) fn fulfill_telegram(&mut self, pane: PaneId) {
        for chat in self.telegram.take_for_pane(pane) {
            self.telegram_send_output(chat, pane);
        }
    }

    /// 수신 메시지를 처리: 미지 chat이면 페어링(C1), `/`면 브리지 명령, 아니면 pane 주입.
    pub(crate) fn handle_telegram_msg(&mut self, m: TgMessage) {
        if !self.config.telegram.allowed_chats.contains(&m.chat_id) {
            self.telegram_pair_request(m.chat_id);
            return;
        }
        if let Some(rest) = m.text.trim().strip_prefix('/') {
            self.handle_telegram_command(m.chat_id, rest);
            return;
        }
        // 입력 주입(셸 제어)은 오너(허용 목록 첫 chat) + "모든 권한 부여"가 둘 다 필요.
        // 화이트리스트의 다른 chat은 설정과 무관하게 관찰만(OpenClaw 오너 모델 벤치마킹, C2).
        if self.telegram_owner() != Some(m.chat_id) {
            self.telegram.reply(m.chat_id, "오너 전용 — 셸 입력은 허용 목록의 첫 chat만 가능합니다. (/panes·/use 는 가능)".into());
            return;
        }
        if !self.config.telegram.grant_all {
            self.telegram.reply(m.chat_id, "권한 없음 — 설정 ▸ 텔레그램에서 '모든 권한 부여'를 켜세요. (/panes·/use 는 가능)".into());
            return;
        }
        let Some(pane) = self.telegram_targets.get(&m.chat_id).copied().or_else(|| self.focused_pane()) else {
            self.telegram.reply(m.chat_id, "대상 셸이 없습니다.".into());
            return;
        };
        self.orch.send(nabi_proto::Command::WriteInput {
            pane,
            data: bytes::Bytes::from(format!("{}\r", m.text).into_bytes()),
        });
        let deadline = Instant::now() + std::time::Duration::from_millis(self.config.telegram.idle_timeout_ms.max(500));
        self.telegram.push_pending(m.chat_id, pane, deadline);
    }

    /// 오너 chat = 허용 목록의 첫 항목(제어 권한). 나머지는 관찰 전용.
    pub(crate) fn telegram_owner(&self) -> Option<i64> {
        self.config.telegram.allowed_chats.first().copied()
    }

    /// 브리지 명령 처리(파싱은 nabi_telegram::parse_command, 순수·테스트됨).
    fn handle_telegram_command(&mut self, chat: i64, rest: &str) {
        let reply = match nabi_telegram::parse_command(rest) {
            nabi_telegram::TgCmd::Panes => {
                let cur = self.telegram_targets.get(&chat).map(|p| p.get());
                let mut lines = vec!["\u{1f4cb} panes (/use N 으로 선택):".to_string()];
                if let Ok(panes) = self.orch.panes.read() {
                    let mut ids: Vec<_> = panes.iter().map(|(id, v)| (id.get(), v.title.clone())).collect();
                    ids.sort_by_key(|(id, _)| *id);
                    for (id, title) in ids {
                        lines.push(format!("{id}: {title}{}", if cur == Some(id) { " \u{25c0}" } else { "" }));
                    }
                }
                lines.join("\n")
            }
            nabi_telegram::TgCmd::Use(n) => {
                let pid = self.orch.panes.read().ok().and_then(|m| m.keys().find(|p| p.get() == n).copied());
                if let Some(pid) = pid {
                    self.telegram_targets.insert(chat, pid); // chat별 대상(다인 접근 시 분리).
                    format!("\u{2713} 대상 pane = {n}")
                } else {
                    format!("pane {n} 없음 — /panes 로 확인")
                }
            }
            nabi_telegram::TgCmd::Cancel => {
                // 대상 셸에 Ctrl+C(중단) — 제어 동작이라 오너+grant_all 필요.
                if self.telegram_owner() != Some(chat) {
                    "오너 전용 — 허용 목록의 첫 chat만 가능합니다".to_string()
                } else if !self.config.telegram.grant_all {
                    "권한 없음 — '모든 권한 부여' 필요".to_string()
                } else if let Some(pane) = self.telegram_targets.get(&chat).copied().or_else(|| self.focused_pane()) {
                    self.orch.send(nabi_proto::Command::WriteInput { pane, data: bytes::Bytes::from(vec![0x03u8]) });
                    "\u{2713} Ctrl+C 전송".to_string()
                } else {
                    "대상 셸 없음".to_string()
                }
            }
            nabi_telegram::TgCmd::Help => "명령: /panes(목록) · /use N(대상 선택) · /cancel(Ctrl+C) · /help. 그 외 텍스트는 대상 셸에 입력됩니다.".to_string(),
        };
        self.telegram.reply(chat, reply);
    }
}
