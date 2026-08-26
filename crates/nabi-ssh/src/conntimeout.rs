//! **접속 시간 제한을 정하는 규칙 한 곳.**
//!
//! 15초로 박아 두었던 값이다. 두 가지가 이 값을 서로 다른 쪽으로 잡아당긴다:
//!
//! * **죽은 호스트**에서는 짧아야 한다. 회신이 없는 주소를 1분씩 붙들면 프로그램이 멎은
//!   것처럼 보인다.
//! * **호스트키 확인창**이 뜨는 첫 접속에서는 길어야 한다. 이 시간에는 사용자가 지문을
//!   읽는 시간이 들어가기 때문이다 — 짧게 주면 신뢰를 누른 순간 이미 끊겨 있다.
//!
//! 여기에 **폐쇄망·위성 회선**이 더해진다. 그런 곳에서는 15초가 모자라 붙을 수 있는
//! 서버를 못 붙는다고 답하게 된다. 그래서 사용자가 정할 수 있게 하되, 규칙 자체는
//! 한 곳에 두고 시험한다.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// 설정에서 온 값. keepalive와 **같은 길**로 온다 — 설정이 ssh 층에 닿는 통로를
/// 두 개 만들면 어느 쪽이 이기는지 아무도 모르게 된다.
pub static CONNECT_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(0);

/// 지금 설정된 값으로 이번 접속의 제한을 구한다.
pub fn current(prompting: bool) -> Duration {
    timeout(CONNECT_TIMEOUT_SECS.load(Ordering::Relaxed), prompting)
}

/// 설정에서 온 초. 0이면 "정하지 않았다"는 뜻이라 기본값을 쓴다.
pub const DEFAULT_SECS: u64 = 15;
/// 확인창이 뜰 수 있으면 사람이 읽을 시간을 더 준다.
const PROMPT_SECS: u64 = 180;
/// 너무 짧으면 어떤 서버도 못 붙는다 — 설정 실수로 스스로를 막지 않게 바닥을 둔다.
const MIN_SECS: u64 = 3;
/// 너무 길면 멎은 것과 구별되지 않는다.
const MAX_SECS: u64 = 600;

/// 이번 접속에 줄 시간. `prompting`이면 호스트키 확인창이 뜰 수 있는 접속이다.
pub fn timeout(cfg_secs: u64, prompting: bool) -> Duration {
    let base = match cfg_secs {
        0 => DEFAULT_SECS,
        n => n.clamp(MIN_SECS, MAX_SECS),
    };
    // 확인창이 뜨는 접속은 **더 큰 쪽**을 쓴다. 사용자가 300초로 늘려 뒀다면 그 뜻을
    // 존중하고, 짧게 뒀더라도 지문 읽을 시간은 빼앗지 않는다.
    match prompting {
        true => Duration::from_secs(base.max(PROMPT_SECS)),
        false => Duration::from_secs(base),
    }
}

#[cfg(test)]
mod tests {
    use super::{timeout, DEFAULT_SECS};

    #[test]
    fn zero_means_the_default() {
        assert_eq!(timeout(0, false).as_secs(), DEFAULT_SECS);
    }

    #[test]
    fn a_setting_is_honoured() {
        assert_eq!(timeout(45, false).as_secs(), 45);
    }

    /// **확인창이 뜨는 접속에서 시간을 빼앗지 않는다** — 지문을 읽는 시간이 여기 들어간다.
    #[test]
    fn the_host_key_prompt_always_gets_room_to_read() {
        assert!(timeout(5, true).as_secs() >= 180, "지문 읽을 새도 없이 끊긴다");
        assert!(timeout(0, true).as_secs() >= 180);
        // 더 길게 정해 뒀으면 그 뜻을 따른다.
        assert_eq!(timeout(300, true).as_secs(), 300);
    }

    /// 설정 실수로 스스로를 막지 않는다(0.1초나 하루 같은 값).
    #[test]
    fn absurd_settings_are_clamped() {
        assert!(timeout(1, false).as_secs() >= 3, "너무 짧아 아무 데도 못 붙는다");
        assert!(timeout(86_400, false).as_secs() <= 600, "멎은 것과 구별되지 않는다");
    }
}
