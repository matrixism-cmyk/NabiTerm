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
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, GetStockObject, HFONT, DEFAULT_GUI_FONT};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::win::{BAR_H, BTN_W, ID_ADDR, ID_BACK, ID_FWD, ID_RELOAD, PAD};

// 끼우기 전에 원래 있던 함수를 여기 적어 둔다. 창 하나가 실 하나를 쓰므로 실마다 하나면 된다.
thread_local! {
    static OLD_EDIT_PROC: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
}

/// 이 PC 가 쓰는 **화면 글꼴**을 얻는다.
///
/// ## 왜 필요한가
///
/// 윈도우 기본 컨트롤은 글꼴을 지정하지 않으면 아주 오래된 글꼴로 그린다. 그래서 단추와
/// 주소 칸이 투박해 보였다("UI 가 좀 엉성하다", 사용자 2026-08-29).
///
/// 글꼴 이름을 우리가 박아 두지 않는다 — 윈도우 판·언어·사용자 설정에 따라 다르고,
/// 박아 두면 한글이 안 나오는 PC 가 생긴다. **윈도우에게 물어서** 그 PC 가 쓰는 것을 쓴다.
fn ui_font() -> HFONT {
    use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS};
    let mut m = NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
    // 안전: 크기를 먼저 채워 넘긴다. 실패하면 아래에서 기본 글꼴로 물러난다.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            m.cbSize,
            Some(&mut m as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok();
    if ok {
        // 안전: 방금 윈도우가 채워 준 글꼴 정보다.
        let f = unsafe { CreateFontIndirectW(&m.lfMessageFont) };
        if !f.is_invalid() {
            return f;
        }
    }
    // 안전: 실패해도 그릴 수는 있어야 한다 — 기본 글꼴로 물러난다.
    HFONT(unsafe { GetStockObject(DEFAULT_GUI_FONT) }.0)
}

/// 자식 창에 글꼴을 입힌다.
fn set_font(h: HWND, f: HFONT) {
    // 안전: 손잡이는 방금 만든 자식이고, 글꼴은 우리가 만든 것이다.
    unsafe {
        SendMessageW(h, WM_SETFONT, Some(WPARAM(f.0 as usize)), Some(LPARAM(1)));
    }
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
                BAR_H - PAD * 2,
                Some(parent),
                Some(HMENU(id as *mut core::ffi::c_void)),
                None,
                None,
            )?;
            Ok(())
        };
        // 화살표는 **어느 PC 에서나 그려지는 모양**을 쓴다. 처음에 ← → 를 썼더니 앞으로
        // 단추만 빈 칸으로 나왔다 — 그 글자가 없는 글꼴이었다(화면으로 확인, 2026-08-29).
        // 삼각형은 오래된 글꼴에도 들어 있다.
        btn(ID_BACK, w!("\u{25c0}"))?;
        btn(ID_FWD, w!("\u{25b6}"))?;
        btn(ID_RELOAD, w!("\u{21bb}"))?;
        let addr = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            0,
            0,
            10,
            BAR_H - PAD * 2,
            Some(parent),
            Some(HMENU(ID_ADDR as *mut core::ffi::c_void)),
            None,
            None,
        )?;
        // 안전: 우리가 이 파일에 정의한 함수를 가리킨다 — 모양이 윈도우가 요구하는 것과 같다.
        let f: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = edit_proc;
        // 만든 것들에 이 PC 의 화면 글꼴을 입힌다 — 안 입히면 옛 글꼴로 그려진다.
        let font = ui_font();
        for id in [ID_BACK, ID_FWD, ID_RELOAD, ID_ADDR] {
            if let Ok(h) = GetDlgItem(Some(parent), id as i32) {
                set_font(h, font);
            }
        }
        let old = SetWindowLongPtrW(addr, GWLP_WNDPROC, f as usize as isize);
        OLD_EDIT_PROC.with(|c| c.set(old));
        Ok(addr)
    }
}

/// 창 크기가 바뀌면 도구 줄의 자식들을 다시 놓는다.
pub(crate) fn layout(parent: HWND, width: i32) {
    // 안전: 손잡이는 이 창의 자식들이고, 없으면 아무 일도 하지 않는다.
    unsafe {
        // 단추 셋을 나란히, 그다음 주소 칸이 남은 자리를 다 쓴다.
        let h_ctl = BAR_H - PAD * 2;
        for (i, id) in [ID_BACK, ID_FWD, ID_RELOAD].into_iter().enumerate() {
            if let Ok(h) = GetDlgItem(Some(parent), id as i32) {
                let _ = MoveWindow(h, PAD + i as i32 * (BTN_W + 4), PAD, BTN_W, h_ctl, true);
            }
        }
        if let Ok(h) = GetDlgItem(Some(parent), ID_ADDR as i32) {
            // 단추 묶음 뒤로 한 칸 더 띄운다 — 붙어 있으면 어디까지가 단추인지 헷갈린다.
            let x = PAD + 3 * (BTN_W + 4) + PAD;
            let _ = MoveWindow(h, x, PAD, (width - x - PAD).max(40), h_ctl, true);
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
        // 안전: `old` 는 우리가 끼울 때 윈도우에게 받아 적어 둔 **원래 함수 주소**다.
        // 다른 값이 들어올 길이 없고(이 실의 thread_local), 0 이면 위에서 빠져나갔다.
        let f: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            std::mem::transmute(old);
        f(hwnd, msg, wp, lp)
    }
}
