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
    /// WebView2 가 만든 창 — 여기에 **보일 영역**을 지정해 덮인 곳만 도려낸다.
    /// 못 찾았으면 `None` 이고, 그때는 예전처럼 통째로 숨긴다.
    host: Option<isize>,
    /// 지금 도려낸 자리. 같은 값이면 다시 시키지 않는다.
    hole: Option<RECT>,
    /// 지금 읽어 오는 중인가.
    ///
    /// 엣지가 알려 주는 것을 여기 적어 둔다 — 물어볼 방법이 없어서다. 새로고침 단추를
    /// 멈춤 단추로 바꾸는 데 쓴다. 읽는 중인지 모르면 멈출 방법이 없다.
    loading: std::rc::Rc<std::cell::Cell<bool>>,
}

impl Embedded {
    /// `parent` 창 안에 웹 화면을 만든다. **부르는 실이 그 창의 UI 실이어야 한다.**
    pub fn create(parent: isize, url: &str) -> Result<Self, String> {
        if !crate::runtime::available() {
            return Err(crate::runtime::INSTALL_HINT.into());
        }
        let hwnd = HWND(parent as *mut core::ffi::c_void);
        let env = crate::view::make_env().map_err(|e| format!("환경을 만들지 못했다: {e}"))?;
        // 만들기 전의 자식 창을 적어 둔다 — 만든 뒤 견줘 새로 생긴 것이 웹 창이다.
        let before = crate::clip::children(hwnd);
        let controller =
            crate::view::make_controller(&env, hwnd).map_err(|e| format!("화면을 붙이지 못했다: {e}"))?;
        // 안전: 방금 받은 조종기에서 화면을 꺼낸다.
        let webview = unsafe { controller.CoreWebView2() }.map_err(|e| format!("화면을 얻지 못했다: {e}"))?;
        let loading = std::rc::Rc::new(std::cell::Cell::new(false));
        watch_loading(&webview, &loading);
        let host = crate::clip::new_child(hwnd, &before);
        let me = Self {
            controller,
            webview,
            last: RECT::default(),
            shown: true,
            host,
            hole: None,
            loading,
        };
        me.go(url);
        Ok(me)
    }

    /// 이 자리에 맞춘다. 화면 좌표가 아니라 **부모 창 안에서의 자리**다.
    pub fn place(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let r = RECT { left: x, top: y, right: x + w.max(0), bottom: y + h.max(0) };
        if same(&r, &self.last) {
            return;
        }
        // 자리가 바뀌면 도려낸 것도 뜻이 없어진다 — 다음 `clip` 이 다시 잡도록 비운다.
        if self.hole.is_some() {
            self.hole = None;
            if let Some(host) = self.host {
                crate::clip::punch(host, (0, 0), None);
            }
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
    ///
    /// 셋 다 결과를 버린다 — 갈 곳이 없으면 실패하는데 그것은 정상이고, 사용자에게는
    /// 단추가 아무 일도 안 한 것으로 보이면 충분하다.
    pub fn back(&self) {
        // 안전: 이 실에서 만든 웹 화면을 이 실에서 부른다(WebView2 는 실 하나에 매인다).
        let _ = unsafe { self.webview.GoBack() };
    }
    pub fn forward(&self) {
        // 안전: 위와 같다.
        let _ = unsafe { self.webview.GoForward() };
    }
    pub fn reload(&self) {
        // 안전: 위와 같다.
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

    /// 이 쪽 안에서 **자바스크립트를 실행하고 결과를 받는다.**
    ///
    /// ## 왜 있는가
    ///
    /// pane 안의 AI 가 웹을 읽고 만질 수 있게 하려는 것이다. 지금까지는 열고 옮기는 것만
    /// 됐다 — 무엇이 떠 있는지 알 길이 없으니 AI 에게는 없는 것과 같았다.
    ///
    /// ## 결과는 JSON 이다
    ///
    /// WebView2 는 마지막 식의 값을 **JSON 으로 직렬화해** 돌려준다. 문자열이면 따옴표가
    /// 붙어 오고, `undefined` 면 `"null"` 이 온다. 그대로 넘긴다 — 부르는 쪽이 JSON 으로
    /// 읽으면 되고, 우리가 벗기면 원래 문자열이었는지 알 수 없게 된다.
    ///
    /// ## 답은 나중에 온다
    ///
    /// 지금 자리에서 기다리지 않는다. 기다리면 UI 실이 멈추고, 그 실이 멈추면 화면이
    /// 답을 만들 수 없어 서로 붙잡는다. 그래서 답이 오면 `done` 을 부른다.
    pub fn eval(&self, js: &str, done: impl FnOnce(Result<String, String>) + 'static) {
        let wide: Vec<u16> = js.encode_utf16().chain(std::iter::once(0)).collect();
        // 답이 오는 길과 못 넣었을 때의 길이 **둘 다** 같은 함수를 불러야 한다.
        // 둘 중 하나만 부르도록 상자에 담아 나눠 갖는다.
        let slot = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
        let mine = slot.clone();
        let failed = move |r: Result<String, String>| {
            if let Some(f) = mine.borrow_mut().take() {
                f(r);
            }
        };
        let handler = webview2_com::ExecuteScriptCompletedHandler::create(Box::new(
            move |hr, json| {
                let Some(f) = slot.borrow_mut().take() else { return Ok(()) };
                f(match hr {
                    Ok(()) => Ok(json.to_string()),
                    Err(e) => Err(format!("{e}")),
                });
                Ok(())
            },
        ));
        // 안전: 널로 끝나는 UTF-16 을 넘기고, 손잡이는 이 실에서 만든 화면이다.
        // 넣지 못했으면 답이 영영 오지 않는다 — 부른 쪽이 영원히 기다리게 두지 않는다.
        if let Err(e) = unsafe {
            self.webview.ExecuteScript(windows_core::PCWSTR(wide.as_ptr()), &handler)
        } {
            failed(Err(format!("스크립트를 넣지 못했다: {e}")));
        }
    }

    /// 지금 읽어 오는 중인가.
    pub fn is_loading(&self) -> bool {
        self.loading.get()
    }

    /// 조종 기능(embedctl.rs)이 쓰는 손잡이 — 크레이트 안에서만 보인다.
    pub(crate) fn webview(&self) -> &ICoreWebView2 {
        &self.webview
    }

    /// 같은 이유의 조종기 손잡이. 확대 배율은 화면이 아니라 조종기가 갖고 있다.
    pub(crate) fn controller(&self) -> &ICoreWebView2Controller {
        &self.controller
    }

    /// 이 자리를 **도려낸다**(부모 창 좌표). `None` 이면 온전히 보인다.
    ///
    /// 메뉴나 창이 웹 화면 위에 뜨면 그 자리만 파낸다. 예전에는 통째로 숨겼는데,
    /// 작은 메뉴 하나에 웹 화면이 다 사라져 어색했다(사용자 지적 2026-08-30).
    ///
    /// 웹 창을 못 찾았으면 **아무것도 하지 않는다** — 부르는 쪽이 `can_clip()` 을 보고
    /// 그때는 예전처럼 통째로 숨긴다.
    pub fn clip(&mut self, hole: Option<(i32, i32, i32, i32)>) {
        let Some(host) = self.host else { return };
        // 부모 창 좌표를 웹 창 안의 좌표로 옮긴다.
        let r = hole.map(|(x, y, w, h)| RECT {
            left: x - self.last.left,
            top: y - self.last.top,
            right: x + w - self.last.left,
            bottom: y + h - self.last.top,
        });
        if crate::clip::same_hole(r, self.hole) {
            return;
        }
        self.hole = r;
        let size = (self.last.right - self.last.left, self.last.bottom - self.last.top);
        crate::clip::punch(host, size, r);
    }

    /// 도려낼 수 있는가 — 웹 창을 찾았는가.
    pub fn can_clip(&self) -> bool {
        self.host.is_some()
    }

    /// 지금 보고 있는 쪽의 **제목**. 아직 안 읽혔으면 빈 글.
    ///
    /// 탭 이름에 쓴다. 주소를 그대로 쓰면 `https://github.com/...` 처럼 길어서 탭이
    /// 무엇인지 알아볼 수 없다 — 다른 탭들은 전부 짧은 이름을 달고 있다.
    pub fn title(&self) -> String {
        let mut p = windows_core::PWSTR::null();
        // 안전: 받는 곳은 지역 변수다.
        if unsafe { self.webview.DocumentTitle(&mut p) }.is_err() || p.is_null() {
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

/// 읽기 시작/끝을 엣지에게 들어 두고 깃발에 적는다.
///
/// 엣지에는 "지금 읽는 중이냐"고 물을 길이 없다. 시작할 때와 끝날 때 알려 줄 뿐이다.
/// 그래서 우리가 받아 적어 둔다. 못 걸어도 큰일은 아니다 — 새로고침 단추가 멈춤으로
/// 바뀌지 않을 뿐이라, 실패를 이유로 화면 만들기를 접지는 않는다.
fn watch_loading(webview: &ICoreWebView2, flag: &std::rc::Rc<std::cell::Cell<bool>>) {
    let on = flag.clone();
    let started = webview2_com::NavigationStartingEventHandler::create(Box::new(move |_, _| {
        on.set(true);
        Ok(())
    }));
    let off = flag.clone();
    let done = webview2_com::NavigationCompletedEventHandler::create(Box::new(move |_, _| {
        off.set(false);
        Ok(())
    }));
    // 표는 우리가 뗄 일이 없으니 받아만 두고 버린다(화면이 죽으면 함께 사라진다).
    let mut token = 0i64;
    // 안전: 이 실에서 만든 화면에 이 실에서 만든 처리기를 건다.
    unsafe {
        let _ = webview.add_NavigationStarting(&started, &mut token as *mut _);
        let _ = webview.add_NavigationCompleted(&done, &mut token as *mut _);
    }
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
