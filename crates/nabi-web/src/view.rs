//! WebView2 를 창에 붙이고 주소를 연다.
//!
//! ## 만드는 순서가 두 단계인 이유
//!
//! WebView2 는 **다른 프로세스**에서 돈다(엣지가 그렇게 만들어져 있다). 그래서 만드는 데
//! 시간이 걸리고, 다 되면 알려 주는 방식이다. 두 번 기다린다.
//!
//! ```text
//! 1. 환경을 만든다   — 어느 엣지를 쓸지, 쿠키·캐시를 어디에 둘지 정한다
//! 2. 조종기를 만든다 — 그 환경으로 이 창에 실제 화면을 붙인다
//! ```
//!
//! 기다리는 동안 창 소식을 계속 처리해 줘야 한다. 안 그러면 화면이 멎은 것처럼 보이고,
//! WebView2 쪽도 우리 응답을 기다리다 영영 안 끝난다. `wait_for_async_operation` 이
//! 그 일을 대신 해 준다.
//!
//! ## 실 하나에 창 하나
//!
//! WebView2 는 자기를 만든 실에서만 만질 수 있다. 그래서 창 하나가 실 하나를 통째로 쓰고,
//! 상태를 `thread_local` 에 둔다. 실이 갈리지 않으니 자물쇠가 필요 없다.

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::win::BAR_H;

/// 이 실이 맡은 창의 상태.
///
/// 창 손잡이는 여기 두지 않는다. 부르는 쪽이 늘 들고 오기 때문이다 — 두 군데 두면
/// 언젠가 서로 다른 것을 가리킨다.
struct View {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
}

thread_local! {
    /// 오류 페이지를 띄우는 중인가. 맴도는 것을 막는다.
    static SHOWING_ERROR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static VIEW: std::cell::RefCell<Option<View>> = const { std::cell::RefCell::new(None) };
}

/// 창에 WebView2 를 붙인다. 붙고 나면 `start` 주소를 연다.
pub(crate) fn attach(hwnd: HWND, start: &str) -> Result<(), String> {
    let env = make_env().map_err(|e| format!("환경을 만들지 못했다: {e}"))?;
    let controller = make_controller(&env, hwnd).map_err(|e| format!("화면을 붙이지 못했다: {e}"))?;
    // 안전: 방금 받은 조종기에서 화면을 꺼낸다.
    let webview = unsafe { controller.CoreWebView2() }.map_err(|e| format!("화면을 얻지 못했다: {e}"))?;
    watch_navigation(&webview);
    VIEW.with(|v| *v.borrow_mut() = Some(View { controller, webview }));
    on_resize(hwnd);
    go(hwnd, start);
    Ok(())
}

/// 1단계 — 환경.
fn make_env() -> webview2_com::Result<ICoreWebView2Environment> {
    let (tx, rx) = std::sync::mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        // 안전: 핸들러는 이 호출이 끝날 때까지 살아 있다.
        Box::new(|handler| unsafe {
            // `NABI_WEB_ARGS` 로 엣지에 넘길 인자를 붙인다.
            //
            // 사내망에서 프록시를 지정해야 하거나(`--proxy-server=...`), 그 PC 에서만 나는
            // 문제를 가려내야 할 때 쓴다. 기본은 아무것도 붙이지 않는다 — 우리가 모르는
            // 인자를 몰래 넣어 두면 나중에 왜 그렇게 도는지 아무도 모른다.
            let extra = std::env::var("NABI_WEB_ARGS").unwrap_or_default();
            if extra.is_empty() {
                return CreateCoreWebView2Environment(&handler).map_err(webview2_com::Error::WindowsError);
            }
            let opts = webview2_com::CoreWebView2EnvironmentOptions::default();
            opts.set_additional_browser_arguments(extra);
            let opts: ICoreWebView2EnvironmentOptions = opts.into();
            CreateCoreWebView2EnvironmentWithOptions(
                windows_core::PCWSTR::null(),
                windows_core::PCWSTR::null(),
                &opts,
                &handler,
            )
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |code, env| {
            code?;
            let _ = tx.send(env);
            Ok(())
        }),
    )?;
    rx.recv()
        .ok()
        .flatten()
        .ok_or(webview2_com::Error::TaskCanceled)
}

/// 2단계 — 조종기.
fn make_controller(env: &ICoreWebView2Environment, hwnd: HWND) -> webview2_com::Result<ICoreWebView2Controller> {
    let (tx, rx) = std::sync::mpsc::channel();
    let env = env.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        // 안전: 창 손잡이는 살아 있고, 핸들러는 이 호출 동안 유지된다.
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |code, c| {
            code?;
            let _ = tx.send(c);
            Ok(())
        }),
    )?;
    rx.recv()
        .ok()
        .flatten()
        .ok_or(webview2_com::Error::TaskCanceled)
}

/// 페이지를 못 불러오면 **말한다.**
///
/// 안 그러면 하얀 창만 남는다. 창은 떴고 주소 칸에는 주소가 적혀 있으니 사용자는 우리가
/// 고장 난 줄 안다. 실제로 그렇게 한참 헤맸다 — 프록시인지, 네트워크인지, 우리 잘못인지
/// 알 길이 없었다.
///
/// WebView2 는 실패 이유를 숫자로 준다. 그것을 그대로 화면에 띄운다.
fn watch_navigation(webview: &ICoreWebView2) {
    let handler = webview2_com::NavigationCompletedEventHandler::create(Box::new(|_, args| {
        let Some(args) = args else { return Ok(()) };
        // 안전: 이벤트 인자는 이 호출 동안만 살아 있고 여기서만 읽는다.
        let mut ok = windows_core::BOOL::default();
        // 안전: 받는 곳은 우리 지역 변수다.
        let _ = unsafe { args.IsSuccess(&mut ok) };
        if ok.as_bool() {
            SHOWING_ERROR.with(|f| f.set(false));
            return Ok(());
        }
        let mut why = COREWEBVIEW2_WEB_ERROR_STATUS::default();
        let _ = unsafe { args.WebErrorStatus(&mut why) };
        // 오류 페이지를 띄우면 그것도 "옮겨 감"이라 여기가 또 불린다. 막지 않으면 맴돈다 —
        // 실제로 세 번 불리며 화면이 하얗게 남았다.
        if SHOWING_ERROR.with(|f| f.replace(true)) {
            return Ok(());
        }
        eprintln!("[nabi-web] {} 를 불러오지 못했다 (사유 {})", current_url(), why.0);
        show_error(why.0);
        Ok(())
    }));
    let mut token = windows::Win32::Foundation::HANDLE::default();
    // 안전: 토큰은 우리가 들고 있고, 화면이 사라지면 함께 사라진다.
    let _ = unsafe { webview.add_NavigationCompleted(&handler, &mut token as *mut _ as *mut i64) };
}

/// 지금 화면이 가리키는 주소(진단용).
fn current_url() -> String {
    VIEW.with(|v| {
        let b = v.borrow();
        let Some(view) = b.as_ref() else { return "(아직 없음)".to_string() };
        let mut p = windows_core::PWSTR::null();
        // 안전: 받는 곳은 지역 변수다.
        if unsafe { view.webview.Source(&mut p) }.is_err() || p.is_null() {
            return "(읽지 못함)".into();
        }
        unsafe { p.to_string() }.unwrap_or_default()
    })
}

/// 실패한 자리에 이유를 그린다. 하얀 화면보다 낫다.
fn show_error(code: i32) {
    let html = format!(
        "<body style='font:16px sans-serif;padding:40px;color:#333'>         <h2>페이지를 불러오지 못했습니다</h2>         <p>주소를 다시 확인해 주세요. 사내망이라면 프록시 설정이 필요할 수 있습니다.</p>         <p style='color:#888'>사유 코드 {code}</p></body>"
    );
    let wide: Vec<u16> = html.encode_utf16().chain(std::iter::once(0)).collect();
    VIEW.with(|v| {
        if let Some(view) = v.borrow().as_ref() {
            // 안전: 널로 끝나는 UTF-16 문자열을 넘긴다.
            let _ = unsafe { view.webview.NavigateToString(PCWSTR(wide.as_ptr())) };
        }
    });
}

/// 창 크기가 바뀌었다. 도구 줄을 다시 놓고 웹 화면을 남은 자리에 맞춘다.
pub(crate) fn on_resize(hwnd: HWND) {
    let mut r = RECT::default();
    // 안전: 손잡이는 살아 있는 창이다.
    if unsafe { GetClientRect(hwnd, &mut r) }.is_err() {
        return;
    }
    crate::bar::layout(hwnd, r.right);
    VIEW.with(|v| {
        if let Some(view) = v.borrow().as_ref() {
            let web = RECT { left: 0, top: BAR_H, right: r.right, bottom: r.bottom };
            // 안전: 조종기는 이 실에서 만든 것이고 아직 살아 있다.
            let _ = unsafe { view.controller.SetBounds(web) };
        }
    });
}

/// 주소로 옮겨 간다. 사람이 친 것은 여기서 진짜 주소로 바뀐다.
pub(crate) fn go(hwnd: HWND, input: &str) {
    let target = crate::url::resolve(input);
    let wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    VIEW.with(|v| {
        if let Some(view) = v.borrow().as_ref() {
            // 안전: 넘기는 주소는 널로 끝나는 UTF-16 이다.
            //
            // **결과를 버리지 않는다.** 버렸더니 이동이 아예 시작되지 않았는데도 아무 말이
            // 없어서, 화면이 왜 하얀지 한참 몰랐다.
            if let Err(e) = unsafe { view.webview.Navigate(PCWSTR(wide.as_ptr())) } {
                eprintln!("[nabi-web] {target} 로 옮겨 가지 못했다: {e}");
            }
        }
    });
    crate::bar::set_text(hwnd, &target);
}


/// 도구 줄 단추를 눌렀다.
pub(crate) fn command(hwnd: HWND, id: isize) {
    use crate::win::{ID_ADDR, ID_BACK, ID_FWD, ID_RELOAD};
    VIEW.with(|v| {
        let b = v.borrow();
        let Some(view) = b.as_ref() else { return };
        // 안전: 셋 다 이 실에서 만든 화면에 거는 요청이다.
        unsafe {
            match id {
                ID_BACK => {
                    let _ = view.webview.GoBack();
                }
                ID_FWD => {
                    let _ = view.webview.GoForward();
                }
                ID_RELOAD => {
                    let _ = view.webview.Reload();
                }
                _ => {}
            }
        }
    });
    if id == ID_ADDR {
        go(hwnd, &crate::bar::text(hwnd));
    }
}

/// 이 실이 들고 있던 것을 놓는다. 창이 닫힐 때 부른다.
pub(crate) fn drop_view() {
    VIEW.with(|v| {
        if let Some(view) = v.borrow_mut().take() {
            // 안전: 닫기를 알려 주지 않으면 엣지 프로세스가 남는다.
            let _ = unsafe { view.controller.Close() };
            let _ = view.webview.as_raw();
        }
    });
}
