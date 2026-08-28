//! WebView2 런타임이 **설치되어 있는지** 확인한다.
//!
//! ## 왜 확인이 필요한가
//!
//! 내장 브라우저는 엣지의 화면 그리는 부분(WebView2)을 빌려 쓴다. 크로미움 전체를 우리가
//! 들고 다니지 않아도 되는 대신, 그것이 그 PC 에 있어야 한다.
//!
//! 윈도우 10·11 에는 대개 함께 온다. 그런데 **Windows Server 에는 오지 않는다**(이 개발용
//! PC 가 그렇다). 폐쇄망이 우리 강점인데 그런 곳에도 없을 수 있다.
//!
//! ## 없으면 반드시 말한다
//!
//! 없는데 아무 말이 없으면 창만 열리고 하얗게 남는다. 사용자는 우리 프로그램이 고장 난
//! 줄 안다. 무엇이 없고 어떻게 넣는지까지 알려 준다.
//!
//! **엣지가 설치되어 있어도 소용없다.** 런타임은 엣지와 따로 설치되는 별개의 물건이다.

use webview2_com::Microsoft::Web::WebView2::Win32::GetAvailableCoreWebView2BrowserVersionString;
use windows_core::{PCWSTR, PWSTR};

/// 설치된 런타임 버전. 없으면 `None`.
pub fn version() -> Option<String> {
    let mut out = PWSTR::null();
    // 안전: 널 포인터를 넘기면 "기본 위치에서 찾아라"는 뜻이고, 받는 곳은 우리 지역 변수다.
    let ok = unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut out) };
    if ok.is_err() || out.is_null() {
        return None;
    }
    // 안전: 위에서 널이 아님을 확인했고, 이 문자열은 COM 이 우리에게 넘긴 것이다.
    let s = unsafe { out.to_string() }.ok()?;
    (!s.is_empty()).then_some(s)
}

/// 지금 이 PC 에서 내장 브라우저를 쓸 수 있는가.
pub fn available() -> bool {
    version().is_some()
}

/// 없을 때 사용자에게 무엇을 알려 줄지 — 화면 글은 호출하는 쪽에서 번역한다.
///
/// 여기서는 **넣는 방법만** 돌려준다. 명령 한 줄이면 되는 일을 설명으로 풀어 쓰면
/// 오히려 따라 하기 어렵다.
pub const INSTALL_HINT: &str = "winget install Microsoft.EdgeWebView2Runtime";

#[cfg(test)]
mod tests {
    #[test]
    fn asking_does_not_crash_when_it_is_missing() {
        // 이 PC 에는 설치되어 있지 않다(Windows Server). 없을 때 죽지 않는 것이 요점이다 —
        // 없으면 없다고 말할 수 있어야 창만 열리고 하얗게 남는 일이 없다.
        let _ = super::available();
    }

    #[test]
    fn the_hint_is_a_command_that_can_be_pasted() {
        assert!(super::INSTALL_HINT.starts_with("winget install "));
    }
}
