//! 터널이 **아직 살아 있는지** 지켜본다(배치 AH).
//!
//! 포워딩 네 종류(-L·-R·-D·X11)는 모두 성공한 뒤 `std::future::pending()` 으로 영원히
//! 잠들어 핸들만 붙들고 있었다. 그래서 **SSH 연결이 끊겨도 아무도 몰랐다** — 화면의
//! "활성" 목록에는 죽은 터널이 그대로 남고, 사용자는 되는 줄 알고 그 포트를 쓴다.
//!
//! 2026 년 SSH 사용자 불만 조사에서 "이전 세션이 남긴 죽은 터널"과 "정말 듣고 있는지
//! `lsof`/`netstat` 로 확인해야 한다"가 나란히 꼽힌다. 우리가 그 확인을 대신한다.
//!
//! ## 왜 폴링인가
//!
//! `russh` 의 `Handle::is_closed()` 는 값을 묻는 함수지 기다리는 함수가 아니다. 끊김을
//! 알려 주는 신호가 따로 없으므로 **주기적으로 묻는 수밖에 없다.**
//!
//! ## 얼마나 자주 묻는가
//!
//! 끊긴 터널을 몇 초 더 "활성"으로 보여 주는 것은 큰 해가 아니다. 반대로 자주 깨우면
//! 놀고 있는 터널마다 타이머가 돌아 배터리와 CPU 를 먹는다. **사람이 알아차리기 전에
//! 알려 주면 충분하다**는 선에서 2초로 잡았다 — 사용자가 포트를 써 보고 실패하는 데까지
//! 걸리는 시간보다 짧다.

use std::time::Duration;

/// 살아 있는지 묻는 간격.
pub const POLL: Duration = Duration::from_secs(2);

/// 핸들이 닫힐 때까지 기다린다. 닫히면 돌아온다.
///
/// `is_closed` 를 넘겨받는 이유는 **시험이 진짜 SSH 없이 이 논리를 확인할 수 있게** 하려는
/// 것이다. 실제 호출부는 [`until_closed`] 를 쓰고, 그쪽이 `handle.is_closed()` 를 넘긴다.
///
/// `every` 도 인자인 이유: 시험이 2초를 실제로 기다리게 하면 시험 하나가 몇 초를 먹는다.
/// tokio 의 가상 시계(`test-util`)를 켜는 방법도 있지만, **시험을 위해 배포 의존성을
/// 늘리는 것보다 인자 하나를 두는 편이 싸다.**
pub async fn until_closed_every(every: Duration, mut is_closed: impl FnMut() -> bool) {
    while !is_closed() {
        tokio::time::sleep(every).await;
    }
}

/// 기본 간격으로 지켜본다 — 호출부가 쓰는 문.
pub async fn until_closed(is_closed: impl FnMut() -> bool) {
    until_closed_every(POLL, is_closed).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const FAST: Duration = Duration::from_millis(1);

    #[tokio::test]
    async fn it_returns_as_soon_as_the_handle_closes() {
        // 세 번째 물음에서 닫힌다.
        let n = Arc::new(AtomicUsize::new(0));
        let c = n.clone();
        until_closed_every(FAST, move || c.fetch_add(1, Ordering::Relaxed) >= 2).await;
        assert_eq!(n.load(Ordering::Relaxed), 3, "닫힌 것을 확인한 뒤 바로 돌아온다");
    }

    #[tokio::test]
    async fn an_already_closed_handle_does_not_wait() {
        // 이미 죽은 것을 두고 기다리면 그만큼 거짓 "활성"이 길어진다.
        let t = std::time::Instant::now();
        until_closed_every(Duration::from_secs(30), || true).await;
        assert!(t.elapsed() < Duration::from_secs(1), "한 번도 자지 않아야 한다");
    }

    #[tokio::test]
    async fn a_live_handle_keeps_it_waiting() {
        // 살아 있는 동안은 돌아오지 않아야 한다 — 돌아오면 멀쩡한 터널을 죽었다고 알린다.
        let done = tokio::time::timeout(Duration::from_millis(20), until_closed_every(FAST, || false)).await;
        assert!(done.is_err(), "살아 있으면 계속 기다린다");
    }

    #[test]
    fn the_default_interval_is_short_enough_to_beat_a_person() {
        // 사용자가 포트를 써 보고 실패하는 데까지 걸리는 시간보다 짧아야 뜻이 있다.
        assert!(POLL <= Duration::from_secs(5), "너무 길면 죽은 터널을 계속 활성으로 보여 준다");
        assert!(POLL >= Duration::from_secs(1), "너무 짧으면 놀고 있는 터널마다 CPU 를 먹는다");
    }
}
