//! 기록 재생 — `.cast`를 **원래 속도로** 되감아 본다(배치 Z T2).
//!
//! 기록에 시각을 담은 이유가 이것이다. 줄만 남기면 무엇이 있었는지는 알아도 어떤 속도로
//! 흘렀는지는 잃는다. 장애를 재현할 때 정작 중요한 것이 시간 간격이다.
//!
//! ## 왜 스레드를 쓰지 않는가
//!
//! 재생은 "시각이 되면 다음 덩어리를 넣는" 일이라 타이머가 필요해 보이지만, 이 프로그램은
//! 이미 매 프레임 도는 루프를 가지고 있다. 프레임마다 **지난 시각까지의 덩어리를 한꺼번에**
//! 넣으면 스레드도 채널도 없이 같은 결과가 된다. 스레드를 더하면 pane이 닫힐 때 그 스레드를
//! 어떻게 세울지가 또 하나의 문제가 되는데, 여기서는 그 문제가 아예 생기지 않는다.
//!
//! 화면 갱신 간격보다 촘촘한 사건은 한 프레임에 묶여 들어간다 — 사람 눈에는 같다.

use nabi_types::PaneId;
use std::time::Instant;

/// 재생 중인 기록 하나.
pub(crate) struct Replay {
    /// (경과초, 내용) — 시간순.
    pub events: Vec<(f64, String)>,
    /// 다음에 넣을 사건의 자리.
    pub next: usize,
    /// 재생을 시작한 순간.
    pub began: Instant,
    /// 배속(1.0=원래 속도). 2.0이면 두 배 빠르게.
    pub speed: f32,
    /// 잠시 멈춤 — 멈춘 동안 흐른 시간은 세지 않는다.
    pub paused_at: Option<Instant>,
}

impl Replay {
    pub(crate) fn new(events: Vec<(f64, String)>) -> Self {
        Self { events, next: 0, began: Instant::now(), speed: 1.0, paused_at: None }
    }

    /// 다 넣었는가.
    pub(crate) fn done(&self) -> bool {
        self.next >= self.events.len()
    }

    /// 지금까지 흐른 기록 시각(초). 배속을 반영하고, 멈춘 동안은 흐르지 않는다.
    pub(crate) fn clock(&self) -> f64 {
        let real = match self.paused_at {
            Some(at) => at.duration_since(self.began),
            None => self.began.elapsed(),
        };
        real.as_secs_f64() * self.speed.max(0.01) as f64
    }

    /// 지금 넣어야 할 덩어리들을 떼어 낸다(다음 호출부터는 그 뒤부터).
    ///
    /// 한 번에 여럿이 나올 수 있다 — 프레임 사이에 여러 사건의 시각이 지났을 때다.
    /// 하나씩 프레임마다 넣으면 빠르게 쏟아진 출력이 실제보다 **느리게** 재생된다.
    pub(crate) fn take_due(&mut self) -> Vec<u8> {
        if self.paused_at.is_some() {
            return Vec::new();
        }
        let now = self.clock();
        let mut out = Vec::new();
        while let Some((t, text)) = self.events.get(self.next) {
            if *t > now {
                break;
            }
            out.extend_from_slice(text.as_bytes());
            self.next += 1;
        }
        out
    }
}

/// pane별 재생 상태.
pub(crate) type Replays = std::collections::HashMap<PaneId, Replay>;

#[cfg(test)]
mod tests {
    use super::*;

    fn rp(events: &[(f64, &str)]) -> Replay {
        Replay::new(events.iter().map(|(t, s)| (*t, s.to_string())).collect())
    }

    #[test]
    fn nothing_is_due_before_its_time() {
        let mut r = rp(&[(10.0, "late")]);
        assert!(r.take_due().is_empty(), "10초 뒤 사건이 시작하자마자 나오면 안 된다");
        assert!(!r.done());
    }

    #[test]
    fn everything_already_past_comes_out_at_once() {
        // 프레임 사이에 여러 사건의 시각이 지났을 때 — 하나씩 내보내면 실제보다 느려진다.
        let mut r = rp(&[(0.0, "a"), (0.0, "b"), (0.0, "c"), (99.0, "later")]);
        assert_eq!(r.take_due(), b"abc");
        assert!(!r.done(), "아직 뒤에 하나 남았다");
    }

    #[test]
    fn a_finished_replay_reports_done() {
        let mut r = rp(&[(0.0, "only")]);
        assert_eq!(r.take_due(), b"only");
        assert!(r.done());
        assert!(r.take_due().is_empty(), "끝난 뒤에는 아무것도 나오지 않는다");
    }

    #[test]
    fn pausing_stops_the_clock() {
        let mut r = rp(&[(0.0, "a"), (100.0, "b")]);
        assert_eq!(r.take_due(), b"a");
        r.paused_at = Some(Instant::now());
        assert!(r.take_due().is_empty(), "멈춘 동안에는 아무것도 나오지 않는다");
    }

    #[test]
    fn speed_zero_does_not_divide_by_zero() {
        // 배속을 0으로 만들면 시계가 멈춰야지 터지면 안 된다.
        let mut r = rp(&[(0.0, "a")]);
        r.speed = 0.0;
        assert!(r.clock() >= 0.0);
        assert_eq!(r.take_due(), b"a", "0초 사건은 배속과 무관하게 바로 나온다");
    }

    #[test]
    fn an_empty_recording_is_done_immediately() {
        let mut r = rp(&[]);
        assert!(r.done());
        assert!(r.take_due().is_empty());
    }

    #[test]
    fn hangul_bytes_survive() {
        // 내용을 바이트로 넘기므로 UTF-8이 쪼개지면 안 된다.
        let mut r = rp(&[(0.0, "안녕")]);
        assert_eq!(r.take_due(), "안녕".as_bytes());
    }
}
