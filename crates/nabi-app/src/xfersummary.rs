//! **전송이 다 끝났을 때 한 줄로 알린다.**
//!
//! 큐가 길면 무엇이 어떻게 끝났는지 묻힌다. 항목 하나하나는 상태가 보이지만, 스무 개를
//! 걸어 두고 자리를 비운 사람이 돌아와서 알고 싶은 것은 딱 하나다 — **다 됐나, 뭐가 깨졌나.**
//!
//! ## 언제 말하나
//!
//! 큐가 빈 순간은 이미 `eventsftp`가 `drained`로 잡고 있다 — 여기서 다시 세지 않는다.
//! (처음에는 전환 감지를 따로 만들었다가, 같은 일을 하는 것이 이미 있어 걷어냈다.)
//!
//! 아무것도 실패하지 않았고 한 건뿐이었으면 조용하다 — 파일 하나 받은 것을 굳이 알릴 일이
//! 아니고, 그런 알림이 쌓이면 진짜 알림도 흘려보게 된다.

/// 끝난 묶음의 셈.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub done: usize,
    pub failed: usize,
    pub bytes: u64,
}

impl Summary {
    /// 알릴 만한가 — 실패가 있거나, 두 건 이상 끝났을 때.
    pub(crate) fn worth_saying(&self) -> bool {
        self.failed > 0 || self.done + self.failed >= 2
    }
}

/// 큐 상태에서 셈을 만든다. `(끝남, 실패, 성공한 것의 바이트)`.
pub(crate) fn tally(finished: &[(bool, u64)]) -> Summary {
    let mut s = Summary::default();
    for (ok, bytes) in finished {
        if *ok {
            s.done += 1;
            s.bytes += *bytes;
        } else {
            s.failed += 1;
        }
    }
    s
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_counts_successes_and_failures_apart() {
        let s = tally(&[(true, 100), (false, 0), (true, 50)]);
        assert_eq!((s.done, s.failed, s.bytes), (2, 1, 150));
    }

    /// 실패한 것의 바이트는 세지 않는다 — "얼마를 받았나"에 못 받은 것이 섞이면 안 된다.
    #[test]
    fn a_failed_transfer_adds_no_bytes() {
        let s = tally(&[(false, 999)]);
        assert_eq!(s.bytes, 0);
    }

    /// **파일 하나 받은 것은 알리지 않는다** — 그런 알림이 쌓이면 진짜 알림도 무시하게 된다.
    #[test]
    fn a_single_success_stays_quiet() {
        assert!(!tally(&[(true, 10)]).worth_saying());
    }

    /// 하나라도 실패했으면 개수와 무관하게 말한다.
    #[test]
    fn one_failure_is_always_worth_saying() {
        assert!(tally(&[(false, 0)]).worth_saying());
    }

    #[test]
    fn two_or_more_finishing_together_is_worth_saying() {
        assert!(tally(&[(true, 1), (true, 2)]).worth_saying());
    }


    #[test]
    fn an_empty_queue_says_nothing() {
        assert!(!tally(&[]).worth_saying());
    }
}
