//! 분리 창을 **처음 띄울 때의 크기** — 순수 계산이라 화면 없이 시험한다.
//!
//! ## 왜 따로 두는가
//!
//! 예전에는 820×540 하나였다. 터미널에는 넉넉하지만 **편집기에는 좁다** — 16px 등폭에서
//! 80열이 겨우 들어가고, 줄 번호와 미니맵까지 붙으면 오른쪽이 잘린다(2026-08-31 화면으로
//! 확인. 열어 보면 코드가 `-> (usiz` 에서 끊겼다).
//!
//! 그렇다고 큰 값을 박아 둘 수도 없다. 1366×768 노트북에서 1100×760 을 주면 창이 화면
//! 밖으로 나간다. 그래서 **모니터 크기에서 비율로 정하고 위아래를 자른다.**

/// 처음 띄울 창의 (너비, 높이).
///
/// * `is_editor` — 편집기는 더 넓게. 코드는 가로가 생명이다.
/// * `monitor` — 모니터 크기(모르면 `None`). 모르면 무난한 값으로 간다.
pub(crate) fn first_size(is_editor: bool, monitor: Option<egui::Vec2>) -> [f32; 2] {
    // 바라는 크기와, 그보다 작아지면 쓸모없어지는 하한.
    let (want, floor) = match is_editor {
        true => ([1180.0, 800.0], [720.0, 480.0]),
        false => ([820.0, 540.0], [520.0, 360.0]),
    };
    let Some(m) = monitor.filter(|m| m.x.is_finite() && m.y.is_finite() && m.x > 200.0 && m.y > 200.0)
    else {
        return want;
    };
    // 화면을 다 덮지 않는다 — 뒤의 창이 보여야 옮겨 다닐 수 있다.
    let cap = [m.x * 0.72, m.y * 0.82];
    [
        want[0].min(cap[0]).max(floor[0]).min(m.x - 40.0),
        want[1].min(cap[1]).max(floor[1]).min(m.y - 60.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Option<egui::Vec2> {
        Some(egui::vec2(x, y))
    }

    /// 모니터를 모르면 바라는 값 그대로 — 그래도 편집기가 더 넓어야 한다.
    #[test]
    fn 모니터를_모르면_바라는_값() {
        assert_eq!(first_size(true, None), [1180.0, 800.0]);
        assert_eq!(first_size(false, None), [820.0, 540.0]);
        assert!(first_size(true, None)[0] > first_size(false, None)[0]);
    }

    /// 넓은 화면에서는 바라는 값을 넘지 않는다 — 화면이 크다고 창까지 커질 이유는 없다.
    #[test]
    fn 넓은_화면에서도_바라는_값을_넘지_않는다() {
        assert_eq!(first_size(true, v(3840.0, 2160.0)), [1180.0, 800.0]);
    }

    /// **좁은 화면에서 화면 밖으로 나가지 않는다** — 이것이 이 함수의 존재 이유다.
    #[test]
    fn 좁은_화면에서는_화면_안에_들어온다() {
        let m = egui::vec2(1366.0, 768.0);
        let s = first_size(true, Some(m));
        assert!(s[0] <= m.x - 40.0, "너비가 화면을 넘었다: {s:?}");
        assert!(s[1] <= m.y - 60.0, "높이가 화면을 넘었다: {s:?}");
    }

    /// 아주 작은 화면에서도 쓸 수 없을 만큼 줄어들지는 않는다.
    #[test]
    fn 너무_작아지지는_않는다() {
        let s = first_size(true, v(800.0, 600.0));
        assert!(s[0] >= 720.0 || s[0] >= 800.0 - 40.0, "{s:?}");
        assert!(s[1] > 300.0, "{s:?}");
    }

    /// 이상한 값(0·음수·NaN)이 와도 바라는 값으로 물러난다.
    #[test]
    fn 이상한_모니터_값은_무시한다() {
        for bad in [v(0.0, 0.0), v(-1.0, -1.0), v(f32::NAN, f32::NAN), v(10.0, 10.0)] {
            assert_eq!(first_size(true, bad), [1180.0, 800.0], "{bad:?}");
        }
    }
}
