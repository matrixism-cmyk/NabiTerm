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
    /// "ko"/"ja"/"system"/그 외 코드를 Lang으로(미상은 En).
    pub fn from_code(s: &str) -> Lang {
        match s.to_ascii_lowercase().as_str() {
            "ko" | "kr" | "korean" => Lang::Ko,
            "ja" | "jp" | "japanese" => Lang::Ja,
            _ => Lang::En,
        }
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
