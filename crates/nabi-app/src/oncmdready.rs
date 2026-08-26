//! **복원 명령을 언제 보낼 것인가** — 셸이 받을 준비가 됐는지 판정하는 순수 로직.
//!
//! 워크스페이스 복원은 pane을 띄운 **그 즉시** `on_connect`(예: `claude --continue`)를
//! PTY에 써 넣었다. 그런데 그 순간 셸은 아직 시작도 못 했다 — PowerShell이라면 배너와
//! 프로필 실행 전이고, SSH라면 원격 셸이 rc 파일을 읽기 전이다. 그 사이에 들어간
//! 바이트가 살아남는지는 **가정이었지 보장이 아니었다.**
//!
//! 사용자 보고(2026-08-26): "claude를 켜 둔 채 nabiTerm을 끄고 다시 켜면 pane은 열리는데
//! claude가 다시 뜨지 않는다. SSH도 연결까지는 되는데 거기서 멈춘다." 저장 파일에는
//! 명령이 옵션까지 멀쩡히 들어 있었다(`on_connect = "claude --dangerously-skip-permissions
//! --continue"`). 그러니 남는 자리는 **보내는 시점**뿐이다.
//!
//! ## 무엇을 신호로 삼나
//!
//! "프롬프트가 떴다"를 정확히 알려 주는 것은 셸 통합(OSC 133)인데, **원격 셸에는 그것이
//! 없다.** 우리 설치 스크립트는 윈도우 PowerShell 프로필만 건드린다. 그래서 두 경로 모두에
//! 통하는 신호가 필요하다:
//!
//! 1. **출력이 시작됐다가 잠잠해지면** 보낸다. 셸은 배너·프롬프트를 뱉고 입력을 기다리며
//!    조용해진다. 그 조용함이 곧 "준비됨"이다.
//! 2. 그래도 안 오면 **정해진 시간 뒤에는 그냥 보낸다.** 조용해지지 않는 셸(스피너를 돌리는
//!    로그인 배너 같은)이 있어도 명령을 잃어버리지 않는다.
//!
//! 늦게 보내는 것은 손해가 적지만, 잃어버리는 것은 사용자가 직접 다시 쳐야 한다.

use std::time::Duration;

/// 판정에 필요한 상태(순수 — 시각은 이미 경과 시간으로 환산해 넘긴다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReadyState {
    /// pane이 생긴 뒤 흐른 시간.
    pub age: Duration,
    /// 출력이 한 번이라도 있었나(없으면 셸이 아직 아무 말도 안 한 것).
    pub saw_output: bool,
    /// 마지막 출력 이후 흐른 시간(출력이 없었으면 의미 없음).
    pub quiet: Duration,
}

/// 출력이 멎고 이만큼 지나면 "프롬프트가 떴다"로 본다.
pub(crate) const QUIET: Duration = Duration::from_millis(400);
/// 조용해지지 않아도 이때는 보낸다 — 명령을 잃는 것보다 늦는 것이 낫다.
pub(crate) const DEADLINE: Duration = Duration::from_secs(8);

/// 지금 보내도 되는가.
pub(crate) fn ready(s: ReadyState) -> bool {
    if s.age >= DEADLINE {
        return true; // 최후 보루 — 어떤 셸이든 8초면 입력을 받는다.
    }
    s.saw_output && s.quiet >= QUIET
}

#[cfg(test)]
mod tests {
    use super::{ready, ReadyState, DEADLINE, QUIET};
    use std::time::Duration;

    fn st(age_ms: u64, saw: bool, quiet_ms: u64) -> ReadyState {
        ReadyState {
            age: Duration::from_millis(age_ms),
            saw_output: saw,
            quiet: Duration::from_millis(quiet_ms),
        }
    }

    /// **이것이 버그였다** — 스폰 직후에는 보내지 않는다.
    #[test]
    fn nothing_is_sent_the_instant_the_pane_appears() {
        assert!(!ready(st(0, false, 0)));
        assert!(!ready(st(20, false, 20)), "예전 코드가 정확히 여기서 보냈다");
    }

    /// 셸이 말을 하고 있는 동안에는 기다린다(배너·프로필·로그인 메시지).
    #[test]
    fn we_wait_while_the_shell_is_still_talking() {
        assert!(!ready(st(1_500, true, 50)));
    }

    /// 말이 멎고 잠잠해지면 그때 보낸다.
    #[test]
    fn we_send_once_the_output_goes_quiet() {
        assert!(ready(st(1_500, true, QUIET.as_millis() as u64)));
        assert!(ready(st(1_500, true, 900)));
    }

    /// 출력이 아예 없으면 조용한 것이 아니라 **아직 시작을 안 한 것**이다.
    ///
    /// 이 구분이 없으면 `quiet`가 처음부터 크게 잡혀 스폰 즉시 보내 버린다 — 고치려던
    /// 바로 그 버그로 되돌아간다.
    #[test]
    fn silence_before_the_first_byte_is_not_readiness() {
        assert!(!ready(st(100, false, 100)));
        assert!(!ready(st(3_000, false, 3_000)));
    }

    /// 끝내 조용해지지 않아도 **명령을 잃지는 않는다**.
    #[test]
    fn a_never_quiet_shell_still_gets_the_command() {
        let ms = DEADLINE.as_millis() as u64;
        assert!(!ready(st(ms - 1, true, 10)));
        assert!(ready(st(ms, true, 10)));
        assert!(ready(st(ms, false, 0)), "말이 없는 셸도 마감에는 보낸다");
    }

    /// 마감은 조용함보다 넉넉해야 한다 — 아니면 마감이 늘 먼저 걸려 판정이 무의미해진다.
    #[test]
    fn the_deadline_is_the_last_resort_not_the_usual_path() {
        assert!(DEADLINE > QUIET * 4);
    }
}
