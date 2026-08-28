//! 브라우저가 들어갈 **창을 만든다**.
//!
//! ## 왜 우리가 직접 창을 만드는가
//!
//! WebView2 는 붙일 창 손잡이(HWND)를 달라고 한다. egui 가 만든 창을 줄 수는 없다 —
//! 그 창은 egui 가 매 프레임 자기 그림으로 덮는다. 그래서 이 창은 운영체제에게 직접 받는다.
//!
//! ## 창 구조
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │ ◀ ▶ ⟳ [ 주소를 치는 칸            ]  │  ← 위 40픽셀, 윈도우가 그려 주는 것들
//! ├──────────────────────────────────────┤
//! │                                      │
//! │        WebView2 가 채운다            │
//! │                                      │
//! └──────────────────────────────────────┘
//! ```
//!
//! 단추와 주소 칸은 윈도우가 이미 갖고 있는 것을 쓴다. 직접 그리면 글꼴·색·고대비 설정을
//! 전부 우리가 따라가야 하는데, 그럴 이유가 없다.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 위쪽 도구 줄의 높이(픽셀).
pub(crate) const BAR_H: i32 = 40;
/// 단추 하나의 너비.
pub(crate) const BTN_W: i32 = 34;

/// 자식 창을 알아보는 번호.
pub(crate) const ID_BACK: isize = 1;
pub(crate) const ID_FWD: isize = 2;
pub(crate) const ID_RELOAD: isize = 3;
pub(crate) const ID_ADDR: isize = 4;

/// 창 종류를 한 번만 등록한다. 두 번 등록하면 실패하므로 처음 한 번만 한다.
fn register_once() -> PCWSTR {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let name = w!("nabiWebWindow");
    ONCE.call_once(|| {
        // 안전: 넘기는 구조체는 우리가 채웠고, 창 절차는 이 파일의 함수다.
        unsafe {
            let hinst = GetModuleHandleW(None).unwrap_or_default();
            let cls = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                lpszClassName: name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                    (windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as isize as *mut core::ffi::c_void,
                ),
                ..Default::default()
            };
            RegisterClassW(&cls);
        }
    });
    name
}

/// 창을 하나 만들어 손잡이를 돌려준다.
pub(crate) fn create(title: &str) -> windows::core::Result<HWND> {
    let name = register_once();
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    // 안전: 창 종류는 위에서 등록했고, 제목은 널로 끝나는 UTF-16 이다.
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            name,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1100,
            800,
            None,
            None,
            Some(hinst.into()),
            None,
        )?;
        Ok(hwnd)
    }
}

/// 창에 오는 소식을 받는 곳.
///
/// 여기서는 **크기와 닫기만** 다룬다. 단추를 눌렀을 때 무엇을 할지는 WebView2 를 알아야
/// 하므로 `view.rs` 가 따로 걸어 둔다.
extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // 안전: 윈도우가 부르는 규약대로 받은 값을 그대로 되돌려 준다.
    unsafe {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_SIZE => {
                crate::view::on_resize(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                // 위쪽 16비트가 무슨 일인지, 아래 16비트가 어느 자식인지 알려 준다.
                crate::view::command(hwnd, (wp.0 & 0xffff) as isize);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
