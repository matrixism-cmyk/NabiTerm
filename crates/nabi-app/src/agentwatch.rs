//! 화면 규칙 기반 에이전트 상태 감시(A1) + done 전이(A3).
//!
//! statusLine 훅을 설치하지 않은 에이전트(codex 등)도 상태를 보이게 한다. 훅이 상태를
//! 발행하는 pane은 건드리지 않는다 — 훅이 권위, 화면 감지는 폴백(진실 소스 이원화 방지).

use nabi_agentdetect::{agent_kind, classify, AgentState, Manifest, Screen};
use nabi_types::PaneId;
use std::collections::HashMap;
use std::time::Instant;

/// 감지 주기 — 사람 눈에 즉각이면서 프레임 비용은 무시 가능한 선.
const TICK_MS: u128 = 600;
/// 상태 확정에 필요한 연속 동일 판독 횟수(깜빡임 억제 — herdr의 miss-확인 디바운스).
const CONFIRM: u8 = 2;

pub(crate) struct AgentWatch {
    manifests: Vec<Manifest>,
    /// 정규식이 깨져 버려진 규칙 수(배치 AF) — 조용히 사라지지 않게 화면이 알린다.
    pub dropped: usize,
    last: Instant,
    /// 확정 상태(디바운스 통과).
    pub state: HashMap<PaneId, AgentState>,
    /// 디바운스 후보(상태, 연속 횟수).
    cand: HashMap<PaneId, (AgentState, u8)>,
}

impl AgentWatch {
    /// 내장 규칙 + 사용자 오버라이드(`<설정폴더>/agent-rules/*.toml`, 같은 id면 사용자 승).
    pub fn new(base: Option<&std::path::Path>) -> Self {
        let mut manifests = nabi_agentdetect::builtin();
        if let Some(dir) = base.map(|b| b.join("agent-rules")) {
            for user in nabi_agentdetect::load_dir(&dir) {
                manifests.retain(|m| m.id != user.id);
                manifests.push(user);
            }
        }
        // 사용자가 쓴 규칙 중 정규식이 깨져 버려진 것을 센다(배치 AF).
        //
        // 내장 규칙은 우리가 시험으로 지키니 여기서 셀 이유가 없다. 문제는 **사용자가 쓴
        // 규칙**이다 — 조용히 사라지면 그 사람은 자기 규칙이 왜 안 걸리는지 알 수 없다.
        let dropped = manifests.iter().map(|m| m.dropped).sum();
        Self { manifests, dropped, last: Instant::now(), state: HashMap::new(), cand: HashMap::new() }
    }

    /// 읽어 들인 규칙이 모두 몇 개인가.
    ///
    /// 버린 개수만 말하면 "몇 개 중에서"를 모른다. 특히 규칙 폴더 이름을 잘못 적어
    /// 하나도 안 읽힌 경우, 버린 것도 0 이라 아무 말도 안 하게 된다.
    pub fn rules_loaded(&self) -> usize {
        self.manifests.iter().map(|m| m.rule_count()).sum()
    }

    pub fn manifest(&self, kind: &str) -> Option<&Manifest> {
        self.manifests.iter().find(|m| m.id == kind)
    }

    /// pane이 닫히면 흔적을 지운다(id 재사용 대비).
    pub fn forget(&mut self, pane: PaneId) {
        self.state.remove(&pane);
        self.cand.remove(&pane);
    }

    /// 새 판독을 디바운스에 통과시켜 확정 상태를 갱신한다. 반환=새로 **확정된** 상태.
    fn absorb(&mut self, pane: PaneId, read: AgentState, focused: bool) -> Option<AgentState> {
        let cur = self.state.get(&pane).copied();
        // done은 감지가 아니라 전이 규칙이 만든다: working이었다가 조용해졌는데 아직 안 봤다.
        let read = match (cur, read, focused) {
            (Some(AgentState::Working), AgentState::Idle | AgentState::Unknown, false) => AgentState::Done,
            (Some(AgentState::Done), _, false) => AgentState::Done, // 볼 때까지 유지.
            (Some(AgentState::Done), _, true) => AgentState::Idle,  // 봤다 — 확인 처리.
            _ => read,
        };
        if Some(read) == cur {
            self.cand.remove(&pane);
            return None;
        }
        let n = match self.cand.get(&pane) {
            Some((c, n)) if *c == read => n + 1,
            _ => 1,
        };
        if n < CONFIRM {
            self.cand.insert(pane, (read, n));
            return None;
        }
        self.cand.remove(&pane);
        self.state.insert(pane, read);
        Some(read)
    }
}

impl crate::app::NabiApp {
    /// 매 프레임 호출(내부 600ms 스로틀). blocked 전이 pane은 토스트로 알린다.
    pub(crate) fn tick_agent_watch(&mut self) {
        if self.agent_watch.last.elapsed().as_millis() < TICK_MS {
            return;
        }
        self.agent_watch.last = Instant::now();
        // 상태 키 TTL(B7): 만료된 키를 삭제(state 키였다면 권위 반납 → 감지가 다시 맡는다).
        let now = Instant::now();
        let expired: Vec<_> = self.pane_status_ttl.iter()
            .filter(|(_, exp)| **exp <= now).map(|(k, _)| k.clone()).collect();
        for (pid, key) in expired {
            self.pane_status_ttl.remove(&(pid, key.clone()));
            self.set_pane_status(pid, key, None);
        }
        let focused = self.focused_pane();
        let mut newly_blocked: Vec<PaneId> = Vec::new();
        let panes = self.orch.panes.read().ok().map(|m| {
            m.iter().map(|(p, v)| (*p, v.model.clone(), v.title.clone())).collect::<Vec<_>>()
        });
        for (pane, model, title) in panes.unwrap_or_default() {
            // 훅이 상태를 발행하는 pane은 훅이 권위 — 화면 감지를 겹치지 않는다.
            if self.pane_status.get(&pane).is_some_and(|st| st.contains_key("state")) {
                self.agent_watch.forget(pane);
                continue;
            }
            let Some(kind) = self.run_cmd.get(&pane).and_then(|c| agent_kind(c)) else {
                self.agent_watch.forget(pane);
                continue;
            };
            let Some(read) = self.agent_watch.manifest(kind).map(|m| {
                let bottom = model.lock().ok().map(|md| md.visible_bottom_text(4)).unwrap_or_default();
                classify(m, &Screen { bottom: &bottom, title: &title }).0
            }) else {
                continue;
            };
            if let Some(new) = self.agent_watch.absorb(pane, read, focused == Some(pane)) {
                // 상태 확정 → 제어 평면(agent wait/이벤트 구독)에 합성 이벤트 발행(B1).
                self.control_events.publish(&nabi_proto::Event::AgentStatus {
                    pane,
                    state: state_name(new),
                });
                if new == AgentState::Blocked && focused != Some(pane) {
                    newly_blocked.push(pane);
                }
            }
        }
        for pane in newly_blocked {
            // 발행 상태 경로(controlui)와 같은 중복 억제 맵을 공유한다.
            if !self.blocked_alert.insert(pane, true).unwrap_or(false) {
                let title = self.orch.panes.read().ok()
                    .and_then(|m| m.get(&pane).map(|v| v.title.clone()))
                    .unwrap_or_default();
                let word = nabi_i18n::tr(self.lang, "ai.state.blocked");
                self.notify = Some((format!("\u{23f8} {title}: {word}"), Instant::now()));
                if self.config.terminal.agent_sound {
                    crate::bell::system_beep(); // 입력 대기 전이음(A7, opt-in).
                }
            }
        }
    }

    /// 표시용 병합 상태: 발행 상태(권위)가 있으면 그걸, 없으면 화면 감지를 쓴다.
    /// 0=idle · 1=working · 2=blocked · 3=done(완료·미확인).
    pub(crate) fn merged_agent_state(
        &self,
        pane: PaneId,
        st: &std::collections::BTreeMap<String, String>,
        running: bool,
    ) -> u8 {
        if st.contains_key("state") {
            return crate::aistatus::agent_state(st, running); // 훅 발행이 권위.
        }
        match self.agent_watch.state.get(&pane) {
            Some(AgentState::Working) => 1,
            Some(AgentState::Blocked) => 2,
            Some(AgentState::Done) => 3,
            Some(AgentState::Idle) => 0,
            _ => crate::aistatus::agent_state(st, running), // 감지도 없으면 기존 폴백.
        }
    }
}

/// 상태 이름(제어 평면 어휘 — agent wait --until 과 일치).
pub(crate) fn state_name(s: AgentState) -> &'static str {
    match s {
        AgentState::Idle => "idle",
        AgentState::Working => "working",
        AgentState::Blocked => "blocked",
        AgentState::Done => "done",
        AgentState::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch() -> AgentWatch {
        AgentWatch::new(None)
    }

    /// 한 번 읽힌 상태는 확정되지 않는다(깜빡임 억제) — 두 번 연속이어야 바뀐다.
    #[test]
    fn debounce_requires_two_consecutive_reads() {
        let mut w = watch();
        let p = PaneId::new(1);
        assert_eq!(w.absorb(p, AgentState::Working, false), None);
        assert_eq!(w.state.get(&p), None, "1회차는 후보일 뿐");
        assert_eq!(w.absorb(p, AgentState::Working, false), Some(AgentState::Working));
        assert_eq!(w.state.get(&p), Some(&AgentState::Working));
        // 다른 상태가 끼어들면 카운트가 리셋된다.
        assert_eq!(w.absorb(p, AgentState::Idle, false), None);
        assert_eq!(w.absorb(p, AgentState::Blocked, false), None);
        assert_eq!(w.state.get(&p), Some(&AgentState::Working), "연속이 아니면 유지");
    }

    /// working이었다가 조용해졌는데 안 보고 있었다면 done — 보면 idle로 확인 처리.
    #[test]
    fn done_transition_until_seen() {
        let mut w = watch();
        let p = PaneId::new(2);
        w.absorb(p, AgentState::Working, false);
        w.absorb(p, AgentState::Working, false);
        w.absorb(p, AgentState::Idle, false);
        w.absorb(p, AgentState::Idle, false);
        assert_eq!(w.state.get(&p), Some(&AgentState::Done));
        // 아직 안 봤으면 어떤 판독이 와도 done 유지.
        w.absorb(p, AgentState::Idle, false);
        w.absorb(p, AgentState::Idle, false);
        assert_eq!(w.state.get(&p), Some(&AgentState::Done));
        // 포커스하면 idle로.
        w.absorb(p, AgentState::Idle, true);
        w.absorb(p, AgentState::Idle, true);
        assert_eq!(w.state.get(&p), Some(&AgentState::Idle));
    }

    /// 확정 순간에만 새 상태를 돌려준다(같은 상태 반복은 None — 이벤트 중복 방지).
    #[test]
    fn confirmation_fires_once() {
        let mut w = watch();
        let p = PaneId::new(3);
        assert_eq!(w.absorb(p, AgentState::Blocked, false), None, "1회차는 후보");
        assert_eq!(w.absorb(p, AgentState::Blocked, false), Some(AgentState::Blocked));
        assert_eq!(w.absorb(p, AgentState::Blocked, false), None, "유지 중엔 재발행 없음");
    }
}
