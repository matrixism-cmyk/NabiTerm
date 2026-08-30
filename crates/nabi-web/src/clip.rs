//! 웹 화면에서 **덮인 부분만 오려낸다**.
//!
//! ## 왜 필요한가
//!
//! 웹 화면은 운영체제가 자기 창에 그린다. 그 창은 우리가 그리는 것보다 늘 위에 온다.
//! 그래서 메뉴나 설정 창이 뜨면 그것이 웹 화면 뒤로 들어가 보이지 않았다.
//!
//! 처음에는 **웹 화면을 통째로 숨겼다.** 보이기는 했지만, 작은 메뉴 하나 때문에 웹 화면
//! 전체가 사라졌다 — 사용자가 "매우 어색하다"고 했다(2026-08-30). 맞는 말이다.
//!
//! ## 무엇을 하는가
//!
//! 창에는 **보일 영역**을 지정할 수 있다(`SetWindowRgn`). 그래서 통째로 숨기는 대신
//! 덮인 자리만 도려낸다. 메뉴가 뜬 자리에는 구멍이 나고, 나머지 웹 화면은 그대로 보인다.
//!
//! ## 창을 어떻게 찾는가
//!
//! WebView2 는 자기가 만든 창을 알려 주지 않는다. 그래서 **만들기 전과 후의 자식 창을
//! 견줘** 새로 생긴 것을 찾는다. 못 찾으면 오려내기를 포기하고 예전처럼 통째로 숨긴다 —
//! 찾기에 기대는 방법이라 못 찾는 경우를 반드시 남겨 둔다.

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, RGN_DIFF};
use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;

/// 이 창의 자식 창들을 모은다.
pub(crate) fn children(parent: HWND) -> Vec<isize> {
    let mut out: Vec<isize> = Vec::new();
    // 안전: 콜백에 넘기는 것은 우리 벡터의 주소이고, 이 함수가 끝나기 전에 회수한다.
    unsafe {
        let _ = EnumChildWindows(Some(parent), Some(push), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

/// `EnumChildWindows` 가 자식마다 부르는 함수.
extern "system" fn push(h: HWND, lp: LPARAM) -> windows::core::BOOL {
    // 안전: 위에서 넘긴 그 벡터다. 다른 값이 올 길이 없다.
    let v = unsafe { &mut *(lp.0 as *mut Vec<isize>) };
    v.push(h.0 as isize);
    true.into()
}

/// `before` 에 없던 자식 창 하나 — WebView2 가 방금 만든 것이다.
pub(crate) fn new_child(parent: HWND, before: &[isize]) -> Option<isize> {
    children(parent).into_iter().find(|h| !before.contains(h))
}

/// 창에서 `hole` 만큼을 도려낸다. `hole` 이 없으면 다시 온전히 보이게 한다.
///
/// 좌표는 **그 창 안에서의 자리**다(왼쪽 위가 0,0).
pub(crate) fn punch(host: isize, size: (i32, i32), hole: Option<RECT>) {
    let h = HWND(host as *mut core::ffi::c_void);
    let Some(hole) = hole else {
        // 안전: 영역을 없애면 창이 원래대로 다 보인다.
        unsafe {
            SetWindowRgn(h, None, true);
        }
        return;
    };
    // 안전: 만든 영역은 아래에서 창에 넘기거나(그러면 창이 갖는다) 우리가 지운다.
    unsafe {
        let full = CreateRectRgn(0, 0, size.0, size.1);
        let cut = CreateRectRgn(hole.left, hole.top, hole.right, hole.bottom);
        let ok = CombineRgn(Some(full), Some(full), Some(cut), RGN_DIFF);
        let _ = DeleteObject(cut.into());
        if ok.0 == 0 {
            let _ = DeleteObject(full.into());
            return; // 못 만들었으면 건드리지 않는다 — 잘못 넘기면 창이 사라진다.
        }
        // 넘긴 뒤에는 창이 갖는다 — 우리가 지우면 안 된다.
        if SetWindowRgn(h, Some(full), true) == 0 {
            let _ = DeleteObject(full.into());
        }
    }
}

/// 오려낼 영역이 지난번과 같은가 — 같으면 다시 시키지 않는다(매 프레임 부르는 자리다).
pub(crate) fn same_hole(a: Option<RECT>, b: Option<RECT>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.left == y.left && x.top == y.top && x.right == y.right && x.bottom == y.bottom
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::same_hole;
    use windows::Win32::Foundation::RECT;

    fn r(l: i32, t: i32, rt: i32, b: i32) -> RECT {
        RECT { left: l, top: t, right: rt, bottom: b }
    }

    #[test]
    fn 같은_구멍은_다시_시키지_않는다() {
        assert!(same_hole(None, None));
        assert!(same_hole(Some(r(1, 2, 3, 4)), Some(r(1, 2, 3, 4))));
        assert!(!same_hole(Some(r(1, 2, 3, 4)), Some(r(1, 2, 3, 5))));
        assert!(!same_hole(None, Some(r(1, 2, 3, 4))));
        assert!(!same_hole(Some(r(1, 2, 3, 4)), None));
    }
}
