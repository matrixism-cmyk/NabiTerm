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

/// 몇 번까지·얼마나 오래 기다릴지는 **설정에서 온다**(`terminal.reconnect_*`).
///
/// 예전에는 두 값이 코드에 박혀 있었다. 그런데 알맞은 값은 회선마다 다르다 — 사무실 유선은
/// 두 번이면 충분하고, 자주 끊기는 무선·VPN은 더 오래 버텨 주는 편이 낫다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    /// 최대 몇 번까지 시도하는가(첫 시도 포함). 0이면 자동 재접속을 하지 않는다.
    pub tries: u32,
    /// 재시도 간격의 상한(초).
    pub cap_secs: u64,
}

impl Policy {
    /// 설정에서 읽는다. 상한이 0이면 간격이 없어져 서버를 두드리게 되므로 1초를 하한으로 둔다.
    pub fn from_cfg(cfg: &nabi_config::schema::TerminalCfg) -> Self {
        Self { tries: cfg.reconnect_max_tries, cap_secs: cfg.reconnect_max_wait_secs.max(1) }
    }
}

/// 다음 시도까지 기다릴 초. 1 → 2 → 4 → 8 → … 하다가 `cap`에서 멈춘다.
///
/// 지수로 늘리되 상한을 둔다. 너무 길어지면 사용자는 프로그램이 멈춘 줄 안다.
pub(crate) fn delay_secs(tries: u32, cap: u64) -> u64 {
    let cap = cap.max(1);
    match tries {
        0 => 1.min(cap),
        n if n >= 63 => cap,
        n => (1u64 << n).min(cap),
    }
}

impl Backoff {
    pub fn first() -> Self {
        Self { tries: 0 }
    }

    /// 한 번 더 시도할 수 있는가.
    pub fn may_retry(&self, p: Policy) -> bool {
        self.tries < p.tries
    }

    /// 다음 시도까지 기다릴 시간.
    pub fn wait(&self, p: Policy) -> std::time::Duration {
        std::time::Duration::from_secs(delay_secs(self.tries, p.cap_secs))
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
        let d: Vec<u64> = (0..6).map(|t| delay_secs(t, 15)).collect();
        assert_eq!(d, vec![1, 2, 4, 8, 15, 15]);
        for w in d.windows(2) {
            assert!(w[1] >= w[0], "간격이 줄었다: {d:?}");
        }
    }

    /// **상한이 있어야 한다** — 한없이 늘면 사용자는 멈춘 줄 안다.
    #[test]
    fn the_wait_is_capped() {
        assert!(delay_secs(100, 15) <= 15);
        assert!(delay_secs(100, 60) <= 60);
    }

    /// 설정한 상한을 그대로 따른다 — 코드에 박힌 15초가 아니라.
    #[test]
    fn 설정한_상한을_따른다() {
        assert_eq!(delay_secs(9, 5), 5, "상한 5초를 넘었다");
        assert_eq!(delay_secs(3, 60), 8, "상한이 넉넉하면 지수 그대로");
        // 상한을 0으로 적어 두어도 간격이 사라지면 안 된다(서버를 두드린다).
        assert!(delay_secs(0, 0) >= 1);
        // 아주 큰 시도 횟수에서 시프트가 넘치면 안 된다.
        assert_eq!(delay_secs(u32::MAX, 15), 15);
    }

    /// 최대 시도 수도 설정에서 온다. 0이면 아예 시도하지 않는다.
    #[test]
    fn 시도_횟수도_설정에서_온다() {
        let off = Policy { tries: 0, cap_secs: 15 };
        assert!(!Backoff::first().may_retry(off), "0이면 시도하지 않아야 한다");
        let many = Policy { tries: 9, cap_secs: 15 };
        let mut b = Backoff::first();
        for _ in 0..9 {
            assert!(b.may_retry(many));
            b = b.attempted();
        }
        assert!(!b.may_retry(many), "설정한 횟수를 넘겨 시도했다");
    }

    /// 정해진 횟수를 넘으면 멈추고 사용자에게 넘긴다.
    #[test]
    fn it_gives_up_after_a_bounded_number_of_tries() {
        let p = Policy { tries: 5, cap_secs: 15 };
        let mut b = Backoff::first();
        let mut n = 0;
        while b.may_retry(p) {
            b = b.attempted();
            n += 1;
            assert!(n <= 100, "멈추지 않는다");
        }
        assert_eq!(n, p.tries);
    }

    #[test]
    fn a_fresh_backoff_may_retry_and_waits_a_little() {
        let p = Policy { tries: 5, cap_secs: 15 };
        let b = Backoff::first();
        assert!(b.may_retry(p));
        assert_eq!(b.wait(p).as_secs(), 1);
    }

    /// 시도할수록 남은 기회가 줄어든다.
    #[test]
    fn each_attempt_uses_one_chance() {
        let b = Backoff::first().attempted().attempted();
        assert_eq!(b.tries, 2);
        assert!(b.may_retry(Policy { tries: 5, cap_secs: 15 }));
    }
}
