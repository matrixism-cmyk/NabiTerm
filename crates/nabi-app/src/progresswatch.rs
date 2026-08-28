//! 화면을 읽어 **진행률을 알아서 채운다**(배치 AM).
//!
//! ## 왜 있나
//!
//! 빌드가 몇 분씩 도는 동안 상태 표시줄에는 "1 shell" 밖에 없어 얼마나 남았는지 알 수
//! 없다(사용자 보고 2026-08-28). 진행률을 띄우는 길은 이미 끝까지 이어져 있는데
//! `OSC 9;4` 를 보내는 프로그램이 거의 없다. cargo 도 npm 도 보내지 않는다.
//!
//! 그래서 우리가 대신 읽는다. 무엇을 읽을지는 `nabi_agentdetect::progress` 가 정한다.
//!
//! ## 세 가지를 지킨다
//!
//! 1. **아는 모양만 읽는다.** 아무 퍼센트나 읽으면 막대가 뜻 없이 춤춘다.
//! 2. **뒤로 가는 값은 버린다.** 여러 도구의 출력이 섞이면 값이 오락가락한다.
//! 3. **소식이 끊기면 지운다.** 끝난 작업의 87% 가 남아 있으면 아직 도는 줄 안다.
//!
//! ## 직접 알려 주는 pane 은 건드리지 않는다
//!
//! 프로그램이 `OSC 9;4` 로 제 진행률을 말했다면 그쪽이 맞다. 우리가 화면을 훑어 넘겨짚은
//! 것보다 정확하다. 에이전트 상태에서 "훅이 권위, 화면 감지는 폴백"으로 정한 것과 같다.

use nabi_types::PaneId;
use std::time::{Duration, Instant};

/// 이만큼 새 값이 없으면 지운다. 빌드 출력은 이보다 훨씬 자주 나온다.
const STALE: Duration = Duration::from_secs(20);

/// 화면에서 몇 줄을 훑을지. 진행률은 늘 맨 아래에 있다.
const LINES: usize = 3;

impl crate::app::NabiApp {
    /// 매 프레임 부른다(에이전트 감시와 같은 600ms 박자를 탄다).
    pub(crate) fn tick_progress_watch(&mut self) {
        let panes: Vec<(PaneId, _)> = match self.orch.panes.read() {
            Ok(m) => m.iter().map(|(p, v)| (*p, v.model.clone())).collect(),
            Err(_) => return,
        };
        let now = Instant::now();
        for (pane, model) in panes {
            // 프로그램이 직접 말한 pane 은 그쪽이 권위다.
            if self.progress_osc.contains(&pane) {
                continue;
            }
            let text = match model.lock() {
                Ok(md) => md.visible_bottom_text(LINES),
                Err(_) => continue,
            };
            let read = text.lines().rev().find_map(nabi_agentdetect::progress::read_line);
            match read {
                Some(pct) if nabi_agentdetect::progress::accept(self.progress.get(&pane).copied(), pct) => {
                    self.progress.insert(pane, pct);
                    self.progress_seen.insert(pane, now);
                }
                _ => {
                    // 오래 조용하면 지운다. 끝난 작업의 숫자가 남아 있으면 아직 도는 줄 안다.
                    if self.progress_seen.get(&pane).is_some_and(|t| now.duration_since(*t) > STALE) {
                        self.progress.remove(&pane);
                        self.progress_seen.remove(&pane);
                    }
                }
            }
        }
    }

    /// pane 이 닫혔다. 남은 흔적을 지운다 — pane 번호는 다시 쓰인다.
    pub(crate) fn forget_progress(&mut self, pane: PaneId) {
        self.progress.remove(&pane);
        self.progress_seen.remove(&pane);
        self.progress_osc.remove(&pane);
    }
}
