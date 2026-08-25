//! 창 **위치** 기억·복원 — 껐다 켜도 두던 자리에 뜨게.
//!
//! 크기는 이미 기억하고 있었지만 위치는 아니었다(사용자 요청 2026-08-25). 모니터를 여럿
//! 쓰거나 창을 한쪽에 붙여 쓰는 사람에게는 매번 옮기는 일이 된다.
//!
//! ## 화면 밖으로 나가지 않게
//!
//! 그냥 저장한 값을 되돌려 놓으면 **창이 안 보이는 자리에 뜰 수 있다.** 노트북에서 외부
//! 모니터를 뽑았거나, 해상도가 바뀌었거나, 그 모니터가 이제 왼쪽이 아니라 오른쪽일 때다.
//! 그러면 사용자는 프로그램이 안 켜진 줄 안다.
//!
//! 그래서 복원 전에 **지금 화면 안에 충분히 들어오는지** 보고, 아니면 그냥 기본 자리에
//! 띄운다. "조금 걸쳐 있는 것"은 괜찮다 — 잡아서 끌 수 있으면 된다.

/// 창이 보인다고 인정할 최소 겹침(px). 제목 표시줄을 잡을 수 있을 정도.
const MIN_VISIBLE: f32 = 80.0;

/// 저장된 창 자리(x, y, w, h)가 지금 화면들에서 쓸 만한가.
///
/// `monitors`는 각 모니터의 (x, y, w, h). 하나라도 충분히 겹치면 참.
pub(crate) fn usable(win: (f32, f32, f32, f32), monitors: &[(f32, f32, f32, f32)]) -> bool {
    let (x, y, w, h) = win;
    if w < 200.0 || h < 150.0 {
        return false; // 너무 작으면 저장이 잘못된 것이다.
    }
    monitors.iter().any(|&(mx, my, mw, mh)| {
        let ox = (x + w).min(mx + mw) - x.max(mx);
        let oy = (y + h).min(my + mh) - y.max(my);
        ox >= MIN_VISIBLE && oy >= MIN_VISIBLE
    })
}

/// 설정에 저장된 자리를 복원할지 정한다. 쓸 만하면 (x, y), 아니면 None.
///
/// 모니터 목록은 실제 화면에서 얻는다. 못 얻으면 복원하지 않는다 — 모르면 안전한 쪽으로.
pub(crate) fn restore_pos(cfg: &nabi_config::AppConfig, w: f32, h: f32) -> Option<(f32, f32)> {
    let a = &cfg.appearance;
    let win = (a.window_x, a.window_y, w, h);
    usable(win, &monitors()).then_some((a.window_x, a.window_y))
}

/// 지금 붙어 있는 모니터들의 (x, y, w, h). 못 알아내면 빈 목록.
#[cfg(windows)]
fn monitors() -> Vec<(f32, f32, f32, f32)> {
    // 가상 화면(모든 모니터를 감싸는 사각형) 하나로 본다. 모니터 사이 빈 공간까지
    // 포함하지만, 우리에게 필요한 판정은 "이 자리가 지금 화면 어딘가에 있나"라 충분하다.
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };
    // SAFETY: 시스템 지표를 읽기만 한다(인자 없음, 부작용 없음).
    let (x, y, w, h) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN) as f32,
            GetSystemMetrics(SM_YVIRTUALSCREEN) as f32,
            GetSystemMetrics(SM_CXVIRTUALSCREEN) as f32,
            GetSystemMetrics(SM_CYVIRTUALSCREEN) as f32,
        )
    };
    if w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    vec![(x, y, w, h)]
}

#[cfg(not(windows))]
fn monitors() -> Vec<(f32, f32, f32, f32)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: (f32, f32, f32, f32) = (0.0, 0.0, 1920.0, 1080.0);

    #[test]
    fn a_window_inside_the_screen_is_restored() {
        assert!(usable((100.0, 100.0, 1200.0, 760.0), &[FHD]));
    }

    /// 조금 걸쳐 있는 것은 괜찮다 — 잡아서 끌 수 있으면 된다.
    #[test]
    fn partly_offscreen_is_still_fine() {
        assert!(usable((1800.0, 900.0, 1200.0, 760.0), &[FHD]));
        assert!(usable((-1100.0, -600.0, 1200.0, 760.0), &[FHD]));
    }

    /// **모니터를 뽑았을 때**가 이 함수의 존재 이유다 — 안 보이는 자리면 기본으로 간다.
    #[test]
    fn a_window_on_a_monitor_that_is_gone_is_rejected() {
        // 예전에 두 번째 모니터(오른쪽)에 있던 창.
        let on_second = (2400.0, 200.0, 1200.0, 760.0);
        assert!(usable(on_second, &[FHD, (1920.0, 0.0, 1920.0, 1080.0)]));
        assert!(!usable(on_second, &[FHD]), "모니터가 없으면 복원하지 않는다");
    }

    /// 살짝만 걸친 것은 잡기 어렵다 — 보이는 것으로 치지 않는다.
    #[test]
    fn a_sliver_on_screen_does_not_count() {
        assert!(!usable((1900.0, 500.0, 1200.0, 760.0), &[FHD]), "20px만 보인다");
        assert!(!usable((500.0, 1060.0, 1200.0, 760.0), &[FHD]), "세로로 20px만 보인다");
    }

    /// 저장값이 망가졌으면(0이거나 아주 작으면) 쓰지 않는다.
    #[test]
    fn a_broken_saved_value_is_ignored() {
        assert!(!usable((0.0, 0.0, 0.0, 0.0), &[FHD]));
        assert!(!usable((10.0, 10.0, 50.0, 40.0), &[FHD]));
    }

    /// 모니터 정보를 못 얻었으면 복원하지 않는다(모르면 안전한 쪽).
    #[test]
    fn without_monitor_information_we_do_not_restore() {
        assert!(!usable((100.0, 100.0, 1200.0, 760.0), &[]));
    }
}
