//! 기록 재생을 앱에 잇는다 — 파일 열기, 매 프레임 진행(배치 Z T2).
//!
//! 재생 규칙 자체는 `replay.rs`의 순수 코어에 있고, 여기서는 그것을 화면에 붙인다.
//! 나눈 이유는 규칙을 화면 없이 시험할 수 있게 하려는 것이다 — 시간이 걸린 동작은
//! 눈으로 보면서는 확인하기 어렵다.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_types::PaneId;
use std::time::Instant;

impl NabiApp {
    /// `.cast` 기록을 골라 **보기 전용 pane**에서 재생한다.
    ///
    /// 셸을 띄우지 않는다. 기록을 셸에 흘려 넣으면 그 안의 문자열이 명령으로 실행될 수
    /// 있는데, 남이 준 기록을 여는 순간 그것은 남의 명령을 내 컴퓨터에서 돌리는 일이 된다.
    pub(crate) fn open_replay(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("asciinema", &["cast"])
            .pick_file()
        else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.notify = Some((tr(self.lang, "replay.unreadable").to_string(), Instant::now()));
            return;
        };
        let events = crate::sessioncastread::parse_cast(&text);
        if events.is_empty() {
            // 읽히긴 했는데 그릴 것이 없다 — 형식이 다르거나 빈 기록이다. 빈 창을 띄우고
            // 사용자가 왜 아무것도 안 나오는지 묻게 두는 것보다 그 자리에서 말하는 편이 낫다.
            self.notify = Some((tr(self.lang, "replay.empty").to_string(), Instant::now()));
            return;
        }
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "cast".to_string());
        let secs = crate::sessioncastread::duration(&events);
        self.pending_replay = Some(events);
        self.orch.send(nabi_proto::Command::SpawnViewerPane {
            title: format!("\u{25b6} {title}"),
            size: nabi_types::GridSize::new(120, 32),
            scrollback: self.config.terminal.scrollback,
            reply_seq: None,
        });
        self.notify = Some((
            format!("{} ({secs:.0}s)", tr(self.lang, "replay.started")),
            Instant::now(),
        ));
    }

    /// 새로 생긴 pane이 재생 대상이면 그 pane에 기록을 건다.
    ///
    /// pane은 오케스트레이터가 만들어 `PaneSpawned`로 알려 주므로, 여는 쪽에서는 기록을
    /// 잠시 들고 있다가 여기서 넘긴다.
    pub(crate) fn attach_pending_replay(&mut self, pane: PaneId) -> bool {
        match self.pending_replay.take() {
            Some(events) => {
                self.replays.insert(pane, crate::replay::Replay::new(events));
                true
            }
            None => false,
        }
    }

    /// 매 프레임: 시각이 된 덩어리를 pane에 밀어 넣는다.
    ///
    /// 재생이 없으면 **아무것도 하지 않는다** — 이 함수가 매 프레임 도는 만큼, 평소에
    /// 값을 치르지 않는 것이 중요하다.
    pub(crate) fn step_replays(&mut self, ctx: &egui::Context) {
        if self.replays.is_empty() {
            return;
        }
        let mut finished = Vec::new();
        for (pane, r) in self.replays.iter_mut() {
            let data = r.take_due();
            if !data.is_empty() {
                self.orch.send(nabi_proto::Command::FeedPane { pane: *pane, data });
            }
            if r.done() {
                finished.push(*pane);
            }
        }
        for p in finished {
            self.replays.remove(&p);
        }
        // 재생 중에는 계속 다시 그린다 — 사건 사이가 비어 있어도 시계는 흐른다.
        if !self.replays.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}
