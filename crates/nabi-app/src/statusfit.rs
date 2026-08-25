//! **상태 표시줄이 좁을 때 무엇을 접는가.**
//!
//! 상태바에는 칩이 스무 개 가까이 붙었다. 창을 넓게 쓰면 문제가 없지만, 반으로 줄이면
//! 오른쪽 칩부터 **잘려서 그냥 사라진다.** 시계도 IP도 없어지는데 아무 표시가 없으니
//! 사용자는 그것들이 꺼졌다고 읽는다.
//!
//! 그래서 폭에 따라 단계를 정하고, 접힌 것은 버리지 않고 `⋯` 안에 넣는다.
//! **없애는 것과 접는 것은 다르다** — 접은 것은 한 번 눌러 볼 수 있어야 한다.
//!
//! ## 무엇을 먼저 접는가
//!
//! 지금 하는 일에 가까운 것일수록 남긴다. 제목·세션 수·종료 코드는 마지막까지 남고,
//! 시계처럼 다른 데서도 볼 수 있는 것이 먼저 접힌다.

/// 표시 단계. 큰 값일수록 많이 보인다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tier {
    /// 아주 좁다 — 제목·세션 수·실패 코드만.
    Min = 0,
    /// 좁다 — 지금 하는 일(전송·cwd·인코딩)까지.
    Core = 1,
    /// 보통 — 선택·줌·터널까지.
    Wide = 2,
    /// 넓다 — 전부.
    Full = 3,
}

impl Tier {
    /// 이 단계에서 `want` 이상짜리 칩을 보여 주는가.
    pub(crate) fn shows(self, want: Tier) -> bool {
        self >= want
    }
}

/// 폭(px)에서 단계를 정한다.
///
/// 경계는 창을 실제로 줄여 가며 잡은 값이다. 칩 폭을 재서 정하지 않는 이유는, 글자 폭이
/// 언어와 글꼴에 따라 달라 매 프레임 재면 경계에서 칩이 떨렸다 사라졌다 하기 때문이다.
pub(crate) fn tier(width: f32) -> Tier {
    match width {
        w if w >= 1180.0 => Tier::Full,
        w if w >= 880.0 => Tier::Wide,
        w if w >= 620.0 => Tier::Core,
        _ => Tier::Min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_window_shows_everything() {
        assert_eq!(tier(1600.0), Tier::Full);
        assert!(tier(1600.0).shows(Tier::Full));
    }

    #[test]
    fn a_narrow_window_keeps_only_the_essentials() {
        let t = tier(400.0);
        assert_eq!(t, Tier::Min);
        assert!(t.shows(Tier::Min), "제목과 세션 수는 끝까지 남는다");
        assert!(!t.shows(Tier::Core));
    }

    /// 단계는 폭이 늘수록 **줄지 않는다.** 넓혔는데 칩이 사라지면 고장으로 읽힌다.
    #[test]
    fn widening_never_hides_more() {
        let mut prev = tier(200.0);
        let mut w = 200.0;
        while w < 2000.0 {
            let now = tier(w);
            assert!(now >= prev, "{w}px에서 단계가 낮아졌다");
            prev = now;
            w += 20.0;
        }
    }

    /// 경계 바로 아래·위가 서로 다른 단계여야 한다(경계가 실제로 동작하는가).
    #[test]
    fn each_boundary_actually_switches() {
        assert_ne!(tier(619.0), tier(620.0));
        assert_ne!(tier(879.0), tier(880.0));
        assert_ne!(tier(1179.0), tier(1180.0));
    }

    #[test]
    fn a_zero_width_does_not_panic() {
        assert_eq!(tier(0.0), Tier::Min);
    }
}
