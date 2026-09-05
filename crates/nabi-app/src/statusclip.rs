//! 상태 표시줄 맨 오른쪽 **클립보드 단추** — 윈도우의 클립보드 기록을 연다.
//!
//! 윈도우는 `Win+V` 로 최근에 복사한 것들을 보여 준다. 터미널에서 붙여넣을 것을 고를 때
//! 자주 쓰는데, 그 조합을 아는 사람만 쓴다. 눈에 보이는 자리에 단추를 두면 누구나 쓴다
//! (사용자 요청 2026-09-05).
//!
//! ## 우리가 창을 그리지 않는다
//!
//! 그 목록은 운영체제가 그리는 것이다. 우리가 흉내 내면 두 벌이 되고, 윈도우가 갖고 있는
//! 것(고정·이모지·기기 간 동기화)을 못 따라간다. 그래서 **같은 키를 대신 눌러 준다.**
//!
//! 기록이 꺼져 있으면 윈도우가 스스로 "켜시겠습니까" 창을 띄운다. 그것이 맞는 자리다 —
//! 시스템 설정을 우리가 몰래 바꾸지 않는다.

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_LWIN, VK_V,
};

/// `Win+V` 를 눌렀다 뗀다.
#[cfg(windows)]
pub(crate) fn open_clipboard_history() {
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    // 누르는 순서와 떼는 순서는 반대여야 한다 — 윈도우 키를 먼저 떼면 시작 메뉴가 열린다.
    let seq = [key(VK_LWIN, false), key(VK_V, false), key(VK_V, true), key(VK_LWIN, true)];
    // 안전: 우리 프로세스가 자기 입력 큐에 키를 넣는 것이고, 배열 크기를 정확히 넘긴다.
    unsafe {
        SendInput(&seq, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
pub(crate) fn open_clipboard_history() {}
