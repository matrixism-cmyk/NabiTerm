// clippy 0.1.96 의 `missing_const_for_thread_local` 은 `const` 가 이미 붙어 있어도 경고한다.
// clippy 문서에 실린 예제 그대로 써도 경고하므로 clippy 쪽 결함이다(최소 예제로 확인했다).
// `const` 를 빼도 경고하니 어느 쪽으로도 피할 수 없고, 항목마다 붙이는 allow 는 매크로
// 안까지 닿지 않아 "쓰이지 않은 allow" 가 된다. 그래서 크레이트 전체에 건다.
// 이 판정이 늘 틀리므로 덮어서 잃는 것도 없다. clippy 가 고쳐지면 이 줄을 지운다.
#![allow(clippy::missing_const_for_thread_local)]

//! 내장 브라우저 — 엣지의 WebView2 를 빌려 별도 창으로 웹을 연다.
//!
//! ## 왜 별도 창인가
//!
//! 우리 화면은 egui 로 우리가 직접 그리는데, WebView2 는 운영체제가 자기 창에 그린다.
//! 탭 안에 끼워 넣으면 늘 맨 위에 뜨고, 탭을 옮겨도 따라오지 않고, 화면 밖으로 잘리지도
//! 않는다. nabiPad·SFTP 를 별도 창으로 띄우는 길이 이미 있어서 그것을 그대로 쓴다.
//!
//! ## 우리만 할 수 있는 것
//!
//! SSH 로 서버에 붙고 → 포트 포워딩으로 원격 웹 화면을 끌어와 → 여기서 연다.
//! 로컬 전용 도구는 첫 두 줄을 못 한다.
//!
//! ## 쓰는 법
//!
//! ```no_run
//! nabi_web::open("localhost:8080", "미리 보기");
//! ```
//!
//! 부르는 쪽을 막지 않는다. 창 하나가 실 하나를 쓴다.

mod bar;
pub mod runtime;
pub mod url;
mod view;
mod win;

/// 새 브라우저 창을 연다. 바로 돌아오고, 창은 제 실에서 산다.
///
/// 런타임이 없으면 창을 만들지 않고 `Err` 를 돌려준다 — **빈 창을 띄우지 않는다.**
/// 하얗게 남은 창은 우리 프로그램이 고장 난 것처럼 보인다.
pub fn open(url: &str, title: &str) -> Result<(), String> {
    if !runtime::available() {
        return Err(runtime::INSTALL_HINT.into());
    }
    let (url, title) = (url.to_string(), title.to_string());
    std::thread::Builder::new()
        .name("nabi-web".into())
        .spawn(move || run(&url, &title))
        .map_err(|e| format!("창을 열 실을 만들지 못했다: {e}"))?;
    Ok(())
}

/// 창 하나의 한살이. 창을 만들고, 소식을 다 받고, 닫히면 정리한다.
///
/// **실패하면 반드시 말한다.** 이 실은 부르는 쪽과 떨어져 있어서 여기서 조용히 돌아가면
/// 아무 일도 일어나지 않은 것처럼 보인다. 창이 안 뜨는데 오류도 없으면 어디를 봐야 할지
/// 알 수 없다 — 실제로 그렇게 한 번 헤맸다.
fn run(url: &str, title: &str) {
    if let Err(why) = try_run(url, title) {
        eprintln!("[nabi-web] {why}");
    }
}

fn try_run(url: &str, title: &str) -> Result<(), String> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::WindowsAndMessaging::*;

    // 안전: 이 실은 여기서 시작해서 여기서 끝난다. COM 을 켜고 끄는 짝이 맞는다.
    unsafe {
        // WebView2 는 이 방식(한 실만 쓰는 아파트)을 요구한다.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let hwnd = win::create(title).map_err(|e| format!("창을 만들지 못했다: {e}"))?;
        if let Err(e) = bar::create(hwnd) {
            let _ = DestroyWindow(hwnd);
            CoUninitialize();
            return Err(format!("도구 줄을 만들지 못했다: {e}"));
        }
        let _ = ShowWindow(hwnd, SW_SHOW);
        if let Err(why) = view::attach(hwnd, url) {
            let _ = DestroyWindow(hwnd);
            CoUninitialize();
            return Err(why);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        view::drop_view();
        CoUninitialize();
    }
    Ok(())
}
