//! 웹 화면을 **남의 창 안에** 붙인다(배치 AZ) — 탭 안에 넣기 위한 것.
//!
//! ## 별도 창과 무엇이 다른가
//!
//! 별도 창(`lib.rs::open`)은 우리가 창을 만들고 그 안에서 메시지 루프를 돌린다. 탭은 그럴
//! 수 없다 — 창도 루프도 나비텀의 것이다. 그래서 여기서는 **이미 있는 창에 얹기만** 한다.
//!
//! ## 자식 창이라 생기는 일
//!
//! WebView2 는 운영체제가 자기 창에 그린다. 그 창은 우리가 egui 로 그리는 것보다 **늘 위에**
//! 온다. 그래서 셋을 지켜야 한다.
//!
//! 1. 탭이 보이지 않으면 **숨긴다.** 안 숨기면 다른 탭 위에 웹 화면이 남는다.
//! 2. 메뉴나 팝업이 그 자리를 덮으면 **잠깐 숨긴다.** 안 숨기면 팝업이 웹 화면 아래로 간다.
//! 3. 자리는 매 프레임 맞춘다. 탭을 옮기거나 창 크기를 바꾸면 따라와야 한다.
//!
//! ## 실 문제
//!
//! WebView2 는 자기를 만든 실에서만 만질 수 있다. 여기서는 **나비텀의 UI 실**이 그 실이다 —
//! 만들기도 옮기기도 전부 그 실에서 한다. 별도 창처럼 우리 실을 따로 두지 않는다.

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::Win32::Foundation::{HWND, RECT};

/// 탭 안에 얹은 웹 화면 하나.
pub struct Embedded {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    /// 마지막으로 맞춘 자리. 같은 값이면 다시 맞추지 않는다(매 프레임 부르는 자리다).
    last: RECT,
    /// 지금 보이는가. 같은 값이면 다시 시키지 않는다.
    shown: bool,
}

impl Embedded {
    /// `parent` 창 안에 웹 화면을 만든다. **부르는 실이 그 창의 UI 실이어야 한다.**
    pub fn create(parent: isize, url: &str) -> Result<Self, String> {
        if !crate::runtime::available() {
            return Err(crate::runtime::INSTALL_HINT.into());
        }
        let hwnd = HWND(parent as *mut core::ffi::c_void);
        let env = crate::view::make_env().map_err(|e| format!("환경을 만들지 못했다: {e}"))?;
        let controller =
            crate::view::make_controller(&env, hwnd).map_err(|e| format!("화면을 붙이지 못했다: {e}"))?;
        // 안전: 방금 받은 조종기에서 화면을 꺼낸다.
        let webview = unsafe { controller.CoreWebView2() }.map_err(|e| format!("화면을 얻지 못했다: {e}"))?;
        let me = Self { controller, webview, last: RECT::default(), shown: true };
        me.go(url);
        Ok(me)
    }

    /// 이 자리에 맞춘다. 화면 좌표가 아니라 **부모 창 안에서의 자리**다.
    pub fn place(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = RECT { left: x, top: y, right: x + w.max(0), bottom: y + h.max(0) };
        if same(&r, &self.last) {
            return;
        }
        self.last = r;
        // 안전: 조종기는 이 실에서 만든 것이고 아직 살아 있다.
        let _ = unsafe { self.controller.SetBounds(r) };
    }

    /// 보이게 하거나 숨긴다. 탭이 바뀌거나 팝업이 덮을 때 쓴다.
    pub fn show(&mut self, on: bool) {
        if self.shown == on {
            return;
        }
        self.shown = on;
        // 안전: 위와 같다.
        let _ = unsafe { self.controller.SetIsVisible(on) };
    }

    /// 주소로 옮겨 간다.
    pub fn go(&self, input: &str) {
        let target = crate::url::resolve(input);
        let wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
        // 안전: 널로 끝나는 UTF-16 이다. 결과를 버리지 않는다 — 버렸다가 이동이 시작조차
        // 안 됐는데 아무 말이 없어 한참 헤맸다.
        if let Err(e) = unsafe { self.webview.Navigate(windows::core::PCWSTR(wide.as_ptr())) } {
            eprintln!("[nabi-web] {target} 로 옮겨 가지 못했다: {e}");
        }
    }

    /// 뒤로·앞으로·새로고침.
    pub fn back(&self) {
        let _ = unsafe { self.webview.GoBack() };
    }
    pub fn forward(&self) {
        let _ = unsafe { self.webview.GoForward() };
    }
    pub fn reload(&self) {
        let _ = unsafe { self.webview.Reload() };
    }

    /// 지금 보고 있는 주소.
    pub fn url(&self) -> String {
        let mut p = windows_core::PWSTR::null();
        // 안전: 받는 곳은 지역 변수다.
        if unsafe { self.webview.Source(&mut p) }.is_err() || p.is_null() {
            return String::new();
        }
        unsafe { p.to_string() }.unwrap_or_default()
    }
}

impl Drop for Embedded {
    fn drop(&mut self) {
        // 닫는다고 알려 주지 않으면 엣지 프로세스가 남는다.
        // 안전: 이 실에서 만든 조종기를 이 실에서 닫는다.
        let _ = unsafe { self.controller.Close() };
    }
}

/// 두 자리가 같은가. 매 프레임 부르므로 같으면 아무 일도 하지 않는다.
fn same(a: &RECT, b: &RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

#[cfg(test)]
mod tests {
    use super::same;
    use windows::Win32::Foundation::RECT;

    #[test]
    fn the_same_place_is_recognised() {
        // 매 프레임 부르는 자리라, 같은 값에 일을 시키면 화면이 깜빡인다.
        let a = RECT { left: 1, top: 2, right: 3, bottom: 4 };
        assert!(same(&a, &a.clone()));
    }

    #[test]
    fn a_moved_place_is_different() {
        let a = RECT { left: 1, top: 2, right: 3, bottom: 4 };
        let b = RECT { left: 1, top: 2, right: 3, bottom: 5 };
        assert!(!same(&a, &b));
    }
}
