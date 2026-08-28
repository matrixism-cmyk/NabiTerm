//! 내장 브라우저 창을 연다(배치 AL).
//!
//! ## 왜 여는 일만 따로 두는가
//!
//! 여는 것 자체는 `nabi_web::open` 한 줄이다. 그런데 **열리지 않을 때 무엇을 말할지**가
//! 그보다 길다. 그 판단을 메뉴 처리 한가운데 두면 메뉴 코드가 읽기 어려워진다.
//!
//! ## 열리지 않는 경우
//!
//! WebView2 런타임이 없으면 못 연다. 윈도우 10·11 에는 대개 함께 오지만 Windows Server
//! 에는 오지 않고, 폐쇄망에도 없을 수 있다. **엣지가 설치되어 있어도 별개다.**
//!
//! 그때 "열 수 없습니다"라고만 하면 사용자가 할 수 있는 일이 없다. 그래서 넣는 명령을
//! 그대로 알려 주고, 그 명령은 우리 환경 관리자로도 실행할 수 있다.

/// 시작할 때 보여 줄 곳. 우리 소개 문서다.
const HOME: &str = "https://github.com/matrixism-cmyk/NabiTerm";

/// 브라우저 창을 연다. 화면에 띄울 알림이 있으면 돌려준다.
pub(crate) fn open(lang: nabi_i18n::Lang, url: Option<&str>) -> Option<String> {
    let target = url.unwrap_or(HOME);
    match nabi_web::open(target, nabi_i18n::tr(lang, "web.title")) {
        Ok(()) => None,
        Err(hint) => Some(format!("{} · {hint}", nabi_i18n::tr(lang, "web.noruntime"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_home_page_is_our_own_and_uses_https() {
        // 남의 주소를 기본으로 두면 그쪽이 바뀔 때 우리가 모른다.
        assert!(super::HOME.starts_with("https://github.com/matrixism-cmyk/"));
    }
}
