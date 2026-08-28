//! 위쪽 도구 줄 — 뒤로·앞으로·새로고침 단추와 주소를 치는 칸.
//!
//! 윈도우가 이미 갖고 있는 단추와 입력 칸을 쓴다. 직접 그리면 글꼴·색·고대비 설정을
//! 우리가 전부 따라가야 한다.
//!
//! ## 엔터를 받아 내려고 한 겹 끼운다
//!
//! 윈도우의 입력 칸은 엔터를 **부모에게 알려 주지 않는다.** 원래 대화상자 안에서 쓰라고
//! 만든 것이라, 대화상자가 아니면 엔터가 그냥 사라진다.
//!
//! 그래서 그 칸이 소식을 받는 자리에 우리 함수를 한 겹 끼워 엔터만 가로챈다. 나머지는
//! 원래 함수에게 그대로 넘긴다 — 글자 입력·복사·붙여넣기는 윈도우가 하던 대로 해야 한다.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::win::{BAR_H, BTN_W, ID_ADDR, ID_BACK, ID_FWD, ID_RELOAD};

// 끼우기 전에 원래 있던 함수를 여기 적어 둔다. 창 하나가 실 하나를 쓰므로 실마다 하나면 된다.
thread_local! {
    static OLD_EDIT_PROC: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
}

/// 도구 줄의 자식들을 만든다.
pub(crate) fn create(parent: HWND) -> windows::core::Result<HWND> {
    // 안전: 부모 창은 방금 만든 것이고, 종류 이름은 윈도우가 늘 갖고 있는 것들이다.
    unsafe {
        let btn = |id: isize, text: PCWSTR| -> windows::core::Result<()> {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                text,
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                BTN_W,
                BAR_H - 8,
                Some(parent),
                Some(HMENU(id as *mut core::ffi::c_void)),
                None,
                None,
            )?;
            Ok(())
        };
        btn(ID_BACK, w!("\u{2190}"))?;
        btn(ID_FWD, w!("\u{2192}"))?;
        btn(ID_RELOAD, w!("\u{21bb}"))?;
        let addr = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            0,
            0,
            10,
            BAR_H - 8,
            Some(parent),
            Some(HMENU(ID_ADDR as *mut core::ffi::c_void)),
            None,
            None,
        )?;
        let f: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = edit_proc;
        let old = SetWindowLongPtrW(addr, GWLP_WNDPROC, f as usize as isize);
        OLD_EDIT_PROC.with(|c| c.set(old));
        Ok(addr)
    }
}

/// 창 크기가 바뀌면 도구 줄의 자식들을 다시 놓는다.
pub(crate) fn layout(parent: HWND, width: i32) {
    // 안전: 손잡이는 이 창의 자식들이고, 없으면 아무 일도 하지 않는다.
    unsafe {
        for (i, id) in [ID_BACK, ID_FWD, ID_RELOAD].into_iter().enumerate() {
            if let Ok(h) = GetDlgItem(Some(parent), id as i32) {
                let _ = MoveWindow(h, 4 + i as i32 * (BTN_W + 2), 4, BTN_W, BAR_H - 8, true);
            }
        }
        if let Ok(h) = GetDlgItem(Some(parent), ID_ADDR as i32) {
            let x = 4 + 3 * (BTN_W + 2) + 6;
            let _ = MoveWindow(h, x, 4, (width - x - 4).max(40), BAR_H - 8, true);
        }
    }
}

/// 주소 칸에 글을 넣는다. 사용자가 그 칸을 쓰고 있는 중이면 건드리지 않는다.
///
/// 페이지가 옮겨 갈 때마다 주소를 갱신하는데, 마침 사용자가 다른 주소를 치고 있었다면
/// 방금 친 것이 사라진다. 그것이 가장 짜증스럽다.
pub(crate) fn set_text(parent: HWND, text: &str) {
    // 안전: 넘기는 글은 널로 끝나는 UTF-16 이고, 손잡이는 이 창의 자식이다.
    unsafe {
        let Ok(h) = GetDlgItem(Some(parent), ID_ADDR as i32) else {
            return;
        };
        if windows::Win32::UI::Input::KeyboardAndMouse::GetFocus() == h {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SetWindowTextW(h, PCWSTR(wide.as_ptr()));
    }
}

/// 주소 칸에 쳐 넣은 글을 읽는다.
pub(crate) fn text(parent: HWND) -> String {
    // 안전: 길이를 먼저 물어보고 그만큼만 받는다.
    unsafe {
        let Ok(h) = GetDlgItem(Some(parent), ID_ADDR as i32) else {
            return String::new();
        };
        let n = GetWindowTextLengthW(h);
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..got.max(0) as usize])
    }
}

/// 주소 칸이 소식을 받는 자리에 한 겹 끼운 함수. 엔터만 가로채고 나머지는 넘긴다.
extern "system" fn edit_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // 안전: 원래 함수 주소는 우리가 끼울 때 적어 둔 것이다.
    unsafe {
        if msg == WM_CHAR && wp.0 == b'\r' as usize {
            if let Ok(parent) = GetParent(hwnd) {
                crate::view::go(parent, &text(parent));
            }
            return LRESULT(0); // 삑 소리를 막는다 — 처리했다고 알린다.
        }
        let old = OLD_EDIT_PROC.with(|c| c.get());
        if old == 0 {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        let f: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(old);
        f(hwnd, msg, wp, lp)
    }
}
