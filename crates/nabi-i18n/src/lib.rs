//! nabi-i18n — 경량 다국어(ko/en/ja).
//!
//! 키→문자열 테이블 기반 런타임 전환. egui는 immediate-mode라 매 프레임 tr()을
//! 재평가하므로 전환이 자유롭다. (후속: 복수형/문법이 필요하면 fluent로 확장.)

pub mod catalog;
mod catalog2;
mod catalog3;
mod catalog_editor;
mod catalog_editor2;
mod catalog_sftp;

pub use catalog::tr;

/// 지원 언어.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Ko,
    Ja,
}

impl Lang {
    /// "ko"/"ja"/"system"/그 외 코드를 Lang으로. "system"(기본값)은 OS 표시 언어를 따른다.
    pub fn from_code(s: &str) -> Lang {
        let s = s.to_ascii_lowercase();
        match s.as_str() {
            "ko" | "kr" | "korean" => Lang::Ko,
            "ja" | "jp" | "japanese" => Lang::Ja,
            "en" | "english" => Lang::En,
            // "system"·빈 값·미상 → OS 로케일(한국어 Windows에서 한국어 UI로 시작).
            _ => Self::from_os_locale(),
        }
    }

    /// OS 표시 언어를 Lang으로(감지 실패 시 En).
    fn from_os_locale() -> Lang {
        match os_locale() {
            Some(l) if l.starts_with("ko") => Lang::Ko,
            Some(l) if l.starts_with("ja") => Lang::Ja,
            _ => Lang::En,
        }
    }

    /// 설정 코드가 실제 언어를 고정하는지("system"이 아닌지).
    pub fn is_explicit(code: &str) -> bool {
        matches!(
            code.to_ascii_lowercase().as_str(),
            "ko" | "kr" | "korean" | "ja" | "jp" | "japanese" | "en" | "english"
        )
    }

    /// 언어 자체 표기.
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ko => "한국어",
            Lang::Ja => "日本語",
        }
    }

    /// 전환 메뉴용 순회 목록.
    pub fn all() -> [Lang; 3] {
        [Lang::En, Lang::Ko, Lang::Ja]
    }
}

/// OS 표시 언어 태그(소문자, 예 "ko-kr"). 감지 실패 시 None.
/// 외부 크레이트 의존 없이: Windows는 kernel32 직접 호출, 그 외는 LANG 계열 환경변수.
#[cfg(windows)]
fn os_locale() -> Option<String> {
    const MAX: usize = 85; // LOCALE_NAME_MAX_LENGTH.
    extern "system" {
        fn GetUserDefaultLocaleName(name: *mut u16, len: i32) -> i32;
    }
    let mut buf = [0u16; MAX];
    // 반환값은 NUL을 포함한 문자 수(실패 시 0).
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), MAX as i32) };
    if n <= 1 {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..n as usize - 1]);
    (!s.is_empty()).then(|| s.to_ascii_lowercase())
}

#[cfg(not(windows))]
fn os_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| {
        std::env::var(k)
            .ok()
            .filter(|v| !v.is_empty() && v != "C" && v != "POSIX")
            .map(|v| v.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod lang_tests {
    use super::Lang;

    #[test]
    fn explicit_codes_map_directly() {
        assert_eq!(Lang::from_code("ko"), Lang::Ko);
        assert_eq!(Lang::from_code("Korean"), Lang::Ko);
        assert_eq!(Lang::from_code("ja"), Lang::Ja);
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert!(Lang::is_explicit("ko") && Lang::is_explicit("EN"));
    }

    #[test]
    fn system_defers_to_os_not_english() {
        // "system"·빈 값은 명시 코드가 아니며 OS 로케일 경로를 탄다.
        assert!(!Lang::is_explicit("system"));
        assert!(!Lang::is_explicit(""));
        // 감지 결과는 환경에 따라 다르므로 3개 중 하나이기만 하면 된다.
        assert!(Lang::all().contains(&Lang::from_code("system")));
    }
}
