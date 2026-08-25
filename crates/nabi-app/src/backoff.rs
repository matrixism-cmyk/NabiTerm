//! 재접속 **물러서기** — 한 번 실패하고 포기하지 않는다.
//!
//! 자동 재접속은 있었지만 **한 번만** 시도했다. 노트북이 깨어나는 중이거나 VPN이 잠깐
//! 끊긴 흔한 경우, 그 한 번은 거의 반드시 실패한다. 그러면 사용자는 모달을 본다 —
//! 몇 초만 기다렸다 다시 붙으면 됐을 일에.
//!
//! ## 왜 물러서는가
//!
//! 곧바로 계속 두드리면 서버(또는 그 앞의 방화벽)에 우리가 공격처럼 보인다. fail2ban류는
//! 짧은 시간에 여러 번 실패한 주소를 막는다 — 재접속하려다 오히려 차단당한다. 그래서
//! 간격을 늘려 간다.
//!
//! ## 언제 그만두는가
//!
//! 정해진 횟수를 넘으면 멈추고 사용자에게 넘긴다. 무한히 시도하면 사용자는 무슨 일이
//! 벌어지는지 모른 채 기다리게 되고, 되지 않는 이유(비밀번호가 바뀌었다 등)를 영영 못 본다.

/// 물러서기 상태 — pane 하나에 대해.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Backoff {
    /// 지금까지 시도한 횟수.
    pub tries: u32,
}

/// 최대 몇 번까지 시도하는가(첫 시도 포함).
pub(crate) const MAX_TRIES: u32 = 5;

/// 다음 시도까지 기다릴 초. 1 → 2 → 4 → 8 → 15(상한).
///
/// 지수로 늘리되 상한을 둔다. 15초를 넘기면 사용자는 프로그램이 멈춘 줄 안다.
pub(crate) fn delay_secs(tries: u32) -> u64 {
    const CAP: u64 = 15;
    match tries {
        0 => 1,
        n if n >= 4 => CAP,
        n => (1u64 << n).min(CAP),
    }
}

impl Backoff {
    pub fn first() -> Self {
        Self { tries: 0 }
    }

    /// 한 번 더 시도할 수 있는가.
    pub fn may_retry(&self) -> bool {
        self.tries < MAX_TRIES
    }

    /// 다음 시도까지 기다릴 시간.
    pub fn wait(&self) -> std::time::Duration {
        std::time::Duration::from_secs(delay_secs(self.tries))
    }

    /// 시도했다고 표시한 다음 상태.
    pub fn attempted(self) -> Self {
        Self { tries: self.tries + 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 간격이 늘어나야 한다 — 곧바로 계속 두드리면 차단당한다.
    #[test]
    fn the_wait_grows_between_attempts() {
        let d: Vec<u64> = (0..6).map(delay_secs).collect();
        assert_eq!(d, vec![1, 2, 4, 8, 15, 15]);
        for w in d.windows(2) {
            assert!(w[1] >= w[0], "간격이 줄었다: {d:?}");
        }
    }

    /// **상한이 있어야 한다** — 한없이 늘면 사용자는 멈춘 줄 안다.
    #[test]
    fn the_wait_is_capped() {
        assert!(delay_secs(100) <= 15);
    }

    /// 정해진 횟수를 넘으면 멈추고 사용자에게 넘긴다.
    #[test]
    fn it_gives_up_after_a_bounded_number_of_tries() {
        let mut b = Backoff::first();
        let mut n = 0;
        while b.may_retry() {
            b = b.attempted();
            n += 1;
            assert!(n <= 100, "멈추지 않는다");
        }
        assert_eq!(n, MAX_TRIES);
    }

    #[test]
    fn a_fresh_backoff_may_retry_and_waits_a_little() {
        let b = Backoff::first();
        assert!(b.may_retry());
        assert_eq!(b.wait().as_secs(), 1);
    }

    /// 시도할수록 남은 기회가 줄어든다.
    #[test]
    fn each_attempt_uses_one_chance() {
        let b = Backoff::first().attempted().attempted();
        assert_eq!(b.tries, 2);
        assert!(b.may_retry());
    }
}
