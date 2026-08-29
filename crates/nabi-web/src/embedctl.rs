//! 웹 화면을 **더 깊이 조종하는** 길 — 멈춤·확대·개발자 도구·그림·PDF.
//!
//! ## 왜 나눠 뒀는가
//!
//! `embed.rs` 는 탭 안에 얹고 자리를 맞추는 일을 한다. 여기는 얹은 뒤에 시키는 일이다.
//! 한 파일에 다 넣으면 "붙이는 법"과 "시키는 법"이 섞여 읽기 어렵다.
//!
//! ## 왜 이만큼이 필요한가
//!
//! 자바스크립트(`eval`)로 되는 일은 여기 넣지 않았다 — 누르기·읽기·채우기는 그쪽이 낫다.
//! 여기 있는 것은 **자바스크립트로는 안 되는 것들**이다.
//!
//! * 로딩 멈추기 — 쪽 안에서는 자기 로딩을 멈출 수 없다.
//! * 확대/축소 — 브라우저가 하는 일이지 쪽이 하는 일이 아니다.
//! * 그림·PDF — 쪽 자신을 밖에서 찍는 일이다.
//! * 뒤로/앞으로 **갈 수 있는지** — 단추를 언제 켤지 알아야 한다.
//! * 개발자 도구 — 사람이 직접 들여다볼 때.

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::Interface;

use crate::embed::Embedded;

impl Embedded {
    /// 읽어 오는 중이면 멈춘다.
    pub fn stop(&self) {
        // 안전: 이 실에서 만든 화면이다.
        let _ = unsafe { self.webview().Stop() };
    }

    /// 뒤로 갈 곳이 있는가. 단추를 켤지 정하는 데 쓴다.
    ///
    /// 이걸 안 보면 단추가 늘 켜져 있고, 눌러도 아무 일이 없다 — 고장으로 보인다.
    pub fn can_back(&self) -> bool {
        let mut b = windows::core::BOOL(0);
        // 안전: 받는 곳은 지역 변수다.
        unsafe { self.webview().CanGoBack(&mut b) }.is_ok() && b.as_bool()
    }

    /// 앞으로 갈 곳이 있는가.
    pub fn can_forward(&self) -> bool {
        let mut b = windows::core::BOOL(0);
        // 안전: 받는 곳은 지역 변수다.
        unsafe { self.webview().CanGoForward(&mut b) }.is_ok() && b.as_bool()
    }

    /// 지금 확대 배율(1.0 = 100%).
    pub fn zoom(&self) -> f64 {
        let mut z = 1.0f64;
        // 안전: 받는 곳은 지역 변수다.
        let _ = unsafe { self.controller().ZoomFactor(&mut z) };
        z
    }

    /// 확대 배율을 정한다. 너무 작거나 큰 값은 쓸 수 없는 화면이 되므로 가둔다.
    pub fn set_zoom(&self, z: f64) {
        let z = z.clamp(0.25, 5.0);
        // 안전: 이 실에서 만든 조종기다.
        let _ = unsafe { self.controller().SetZoomFactor(z) };
    }

    /// 개발자 도구를 연다(별도 창 — 엣지가 띄운다).
    pub fn devtools(&self) {
        // 안전: 이 실에서 만든 화면이다.
        let _ = unsafe { self.webview().OpenDevToolsWindow() };
    }

    /// 지금 보이는 화면을 PNG 파일로 찍는다.
    ///
    /// 답을 기다리지 않는다 — `eval` 과 같은 이유다. UI 실이 멈추면 화면이 답을 만들 수
    /// 없어 서로 붙잡는다. 다 찍히면 `done` 을 부른다.
    pub fn capture_png(&self, path: &str, done: impl FnOnce(Result<(), String>) + 'static) {
        let Some(stream) = file_stream(path, true) else {
            done(Err(format!("파일을 만들지 못했다: {path}")));
            return;
        };
        let slot = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
        let mine = slot.clone();
        let handler = webview2_com::CapturePreviewCompletedHandler::create(Box::new(move |hr| {
            if let Some(f) = slot.borrow_mut().take() {
                f(hr.map_err(|e| format!("{e}")));
            }
            Ok(())
        }));
        // 안전: 방금 만든 스트림과 이 실에서 만든 화면을 넘긴다.
        if let Err(e) = unsafe {
            self.webview().CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            )
        } {
            if let Some(f) = mine.borrow_mut().take() {
                f(Err(format!("찍지 못했다: {e}")));
            }
        }
    }

    /// 지금 쪽을 PDF 파일로 뽑는다.
    ///
    /// `CapturePreview` 는 **보이는 만큼만** 찍지만 이것은 쪽 전체를 담는다 — 긴 쪽을
    /// 통째로 남겨야 할 때 쓴다.
    pub fn print_pdf(&self, path: &str, done: impl FnOnce(Result<(), String>) + 'static) {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let slot = std::rc::Rc::new(std::cell::RefCell::new(Some(done)));
        let mine = slot.clone();
        let handler = webview2_com::PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
            if let Some(f) = slot.borrow_mut().take() {
                f(match (hr, ok) {
                    (Ok(()), true) => Ok(()),
                    (Ok(()), false) => Err("엣지가 PDF 를 만들지 못했다".into()),
                    (Err(e), _) => Err(format!("{e}")),
                });
            }
            Ok(())
        }));
        // PDF 는 `ICoreWebView2_7` 부터다. 없으면 낡은 런타임이라는 뜻이다.
        // 안전: 같은 화면을 다른 얼굴로 볼 뿐이다.
        let Ok(v7) = self.webview().cast::<ICoreWebView2_7>() else {
            if let Some(f) = mine.borrow_mut().take() {
                f(Err("이 엣지 런타임은 PDF 저장을 지원하지 않는다".into()));
            }
            return;
        };
        // 안전: 널로 끝나는 UTF-16 경로를 넘긴다. 기본 설정(None)으로 뽑는다.
        if let Err(e) = unsafe {
            v7.PrintToPdf(windows_core::PCWSTR(wide.as_ptr()), None, &handler)
        } {
            if let Some(f) = mine.borrow_mut().take() {
                f(Err(format!("PDF 를 만들지 못했다: {e}")));
            }
        }
    }
}

/// 파일에 쓰는 COM 스트림을 만든다. 있으면 덮어쓴다.
fn file_stream(path: &str, create: bool) -> Option<windows::Win32::System::Com::IStream> {
    use windows::Win32::UI::Shell::SHCreateStreamOnFileW;
    use windows::Win32::System::Com::{STGM_CREATE, STGM_READWRITE};
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mode = match create {
        true => STGM_CREATE.0 | STGM_READWRITE.0,
        false => STGM_READWRITE.0,
    };
    // 안전: 널로 끝나는 UTF-16 경로를 넘기고 받는 곳은 지역 변수다.
    unsafe { SHCreateStreamOnFileW(windows_core::PCWSTR(wide.as_ptr()), mode) }.ok()
}
