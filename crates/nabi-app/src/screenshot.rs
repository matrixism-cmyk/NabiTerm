//! 화면의 한 조각을 **그림 파일로 남긴다**(배치 AN).
//!
//! ## 왜 필요한가
//!
//! 지금까지 에이전트는 pane 의 **글자만** 읽을 수 있었다(`nabi cli capture`). 그런데
//! 글자로는 알 수 없는 것들이 있다 — 그림이 제대로 그려졌는지, 창이 어디에 있는지,
//! 색이 맞는지. 내장 웹 브라우저처럼 우리가 글자로 그리지 않는 것은 아예 볼 수 없다.
//!
//! 그래서 점을 그대로 떠서 PNG 로 남긴다.
//!
//! ## 왜 화면에서 읽는가
//!
//! 창에게 "네 모습을 그려 달라"고 하는 길(`PrintWindow`)도 있지만, 우리 화면은 그래픽
//! 카드가 그리기 때문에 그렇게 물으면 **까맣게 나온다.** 그래서 화면에 이미 나와 있는
//! 것을 그대로 읽는다.
//!
//! 대신 **가려져 있으면 가린 것이 찍힌다.** 다른 창이 위에 있으면 그 창이 나온다.
//! 이건 어쩔 수 없으므로, 찍기 전에 우리 창을 앞으로 부른다.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;

/// 화면 좌표의 사각형을 PNG 로 저장한다.
pub(crate) fn grab(path: &std::path::Path, x: i32, y: i32, w: i32, h: i32) -> Result<(), String> {
    if w <= 0 || h <= 0 {
        return Err(format!("크기가 이상하다: {w}x{h}"));
    }
    // 안전: 얻은 것은 모두 아래에서 짝을 맞춰 놓아 준다. 실패하면 그 자리에서 돌아간다.
    unsafe {
        let screen = GetDC(HWND::default());
        if screen.is_invalid() {
            return Err("화면을 읽을 통로를 얻지 못했다".into());
        }
        let out = copy(screen, x, y, w, h);
        ReleaseDC(HWND::default(), screen);
        let buf = out?;
        nabi_image::shot::save_png(path, w as u32, h as u32, &buf)
    }
}

/// 화면 통로에서 그 자리의 점들을 받아 온다(BGRA → RGBA 로 바꿔서).
///
/// # Safety
/// `screen` 은 살아 있는 화면 DC 여야 한다.
unsafe fn copy(screen: HDC, x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>, String> {
    let mem = CreateCompatibleDC(screen);
    if mem.is_invalid() {
        return Err("옮겨 담을 통로를 만들지 못했다".into());
    }
    let bmp = CreateCompatibleBitmap(screen, w, h);
    if bmp.is_invalid() {
        let _ = DeleteDC(mem);
        return Err("담을 곳을 만들지 못했다".into());
    }
    let old = SelectObject(mem, bmp);
    let ok = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY).is_ok();

    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // 음수로 주면 위에서 아래로 담긴다. 양수면 뒤집혀 나온다.
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let got = GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);

    SelectObject(mem, old);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(mem);

    if !ok {
        return Err("화면을 옮겨 담지 못했다".into());
    }
    if got == 0 {
        return Err("담은 것을 읽어 내지 못했다".into());
    }
    nabi_image::shot::bgra_to_rgba(&mut buf);
    Ok(buf)
}

/// 창이 화면 어디에 있는지.
pub(crate) fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let h = HWND(hwnd as *mut core::ffi::c_void);
    let mut r = windows::Win32::Foundation::RECT::default();
    // 안전: 손잡이가 죽었으면 실패를 돌려준다.
    if unsafe { GetWindowRect(h, &mut r) }.is_err() {
        return None;
    }
    Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
}
