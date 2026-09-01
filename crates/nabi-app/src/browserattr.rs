//! **파일 속성 글자** — 윈도우 탐색기의 "속성" 열과 같은 표기(`R` `H` `S` `A`).
//!
//! ## 왜 글자인가
//!
//! 속성은 켜짐/꺼짐 네 개라 낱말로 적으면 열이 넓어지고 읽기도 느리다. 탐색기가
//! 스무 해 넘게 첫 글자만 적어 온 이유가 그것이다 — 눈이 모양으로 읽는다.
//!
//! ## 순서를 고정한다
//!
//! 파일마다 순서가 달라지면 같은 열인데도 눈이 매번 다시 읽어야 한다. 그래서 켜진 것만
//! 골라 적되 **순서는 언제나 `R H S A`** 다.

/// 읽기 전용.
pub(crate) const READONLY: u32 = 0x0000_0001;
/// 숨김.
pub(crate) const HIDDEN: u32 = 0x0000_0002;
/// 시스템.
pub(crate) const SYSTEM: u32 = 0x0000_0004;
/// 보관(백업 대상 표시).
pub(crate) const ARCHIVE: u32 = 0x0000_0020;

/// 켜진 속성의 글자들. 하나도 없으면 빈 글자열이다(`-` 같은 자리채움을 넣지 않는다 —
/// 대부분의 파일이 그 자리라 화면이 기호로 뒤덮인다).
pub(crate) fn attr_flags(attrs: u32) -> String {
    let mut s = String::new();
    for (bit, ch) in [(READONLY, 'R'), (HIDDEN, 'H'), (SYSTEM, 'S'), (ARCHIVE, 'A')] {
        if attrs & bit != 0 {
            s.push(ch);
        }
    }
    s
}

/// 이 파일시스템 항목의 윈도우 속성 비트. 윈도우가 아니면 0(속성이라는 개념이 없다).
#[cfg(windows)]
pub(crate) fn file_attrs(m: &std::fs::Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    m.file_attributes()
}

#[cfg(not(windows))]
pub(crate) fn file_attrs(m: &std::fs::Metadata) -> u32 {
    let _ = m;
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_bits_that_are_set_get_a_letter() {
        assert_eq!(attr_flags(0), "");
        assert_eq!(attr_flags(READONLY), "R");
        assert_eq!(attr_flags(ARCHIVE), "A");
        assert_eq!(attr_flags(READONLY | ARCHIVE), "RA");
    }

    /// **순서는 비트 순서가 아니라 정해진 순서다** — 파일마다 순서가 달라지면 눈이
    /// 매번 다시 읽어야 한다.
    #[test]
    fn the_order_is_always_the_same() {
        let all = READONLY | HIDDEN | SYSTEM | ARCHIVE;
        assert_eq!(attr_flags(all), "RHSA");
        // 어떤 조합으로 넣어도 같은 순서로 나온다.
        assert_eq!(attr_flags(ARCHIVE | READONLY | SYSTEM), "RSA");
    }

    /// 모르는 비트는 무시한다 — 윈도우는 이 넷 말고도 많은 비트를 쓴다
    /// (압축·인덱싱 제외·재분석 지점 등). 그것까지 적으면 열이 읽을 수 없게 된다.
    #[test]
    fn unknown_bits_are_ignored() {
        assert_eq!(attr_flags(0x0000_0800), "", "압축 비트는 적지 않는다");
        assert_eq!(attr_flags(0x0000_0800 | HIDDEN), "H");
    }
}
