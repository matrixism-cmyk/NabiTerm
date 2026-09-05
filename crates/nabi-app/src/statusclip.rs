//! 상태 표시줄 맨 오른쪽 **클립보드 단추**.
//!
//! ## 처음에는 Win+V 를 대신 눌러 주려 했다 — 안 됐다
//!
//! 윈도우의 클립보드 기록은 `Win+V` 로 열린다. 그 목록은 운영체제가 그리는 것이니 우리가
//! 흉내 내지 말고 같은 키를 보내 주면 되겠다고 생각했다.
//!
//! 실제로 재 보니 안 됐다(2026-09-05). `SendInput` 은 네 개를 다 받아들였는데
//! (돌려준 값 4) 창은 뜨지 않았다. 운영체제가 자기 조합키를 **주입된 입력으로는**
//! 열어 주지 않는다. 이 PC 는 윈도우 서버라 기능 자체가 꺼져 있기도 했다.
//!
//! ## 그래서 우리 것을 보여 준다
//!
//! 우리는 이미 복사한 것들을 기억하고 있었다(`NabiApp::clip_history` — `record_clip` 이
//! 채우고 명령 팔레트가 쓰고 있었다). 그것을 그대로 꺼내 쓴다. 새로 만들 것이 없었다.
//!
//! 우리 것이 오히려 나은 점이 있다.
//!
//! * 윈도우 판·SKU·설정과 상관없이 **어디서나 된다.**
//! * 고른 것을 **지금 보고 있는 pane 에 바로 넣는다** — 창을 옮겨 다닐 필요가 없다.
//! * 원격(SSH) pane 에서도 같다.
//!
//! 윈도우 것을 쓰고 싶은 사람을 위해 목록 맨 아래에 그 길도 남겨 둔다. 그 길이 이 PC 에서
//! 안 열리더라도, 적어도 어디로 가야 하는지는 알 수 있다.

/// 목록에 한 줄로 적을 만큼 줄인다 — 여러 줄이면 첫 줄만, 길면 자른다.
pub(crate) fn one_line(s: &str, max: usize) -> String {
    let first = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    crate::statusfmt::elide(first.trim(), max)
}

/// 윈도우의 클립보드 기록을 열어 본다(`Win+V`).
///
/// 주입으로는 열리지 않는 PC 가 있다 — 위 설명 참고. 그래서 이것은 "되면 좋고"인 길이고,
/// 우리 목록이 본길이다.
#[cfg(windows)]
pub(crate) fn open_windows_clipboard() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        VK_LWIN, VK_V,
    };
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
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
pub(crate) fn open_windows_clipboard() {}

#[cfg(test)]
mod tests {
    #[test]
    fn 목록_한_줄로_줄인다() {
        assert_eq!(super::one_line("가나다", 10), "가나다");
        // 여러 줄이면 첫 줄만.
        assert_eq!(super::one_line("첫 줄\n둘째 줄", 10), "첫 줄");
        // 앞이 비어 있으면 내용이 있는 첫 줄부터.
        assert_eq!(super::one_line("\n\n실제 내용", 10), "실제 내용");
        // 길면 자른다.
        assert_eq!(super::one_line("abcdefghij", 5).chars().count(), 5);
    }
}
