//! nabiPad 전각↔반각 변환(EmEditor/일본어·한국어 에디터 벤치마킹) — CJK 입력 정리용.
//! 전각 영숫자/기호(U+FF01..=U+FF5E)와 전각 공백(U+3000)을 ASCII와 상호 변환한다. 순수 함수.

/// 전각 영숫자·기호·공백을 반각(ASCII)으로. 한글/한자 등 그 외 문자는 유지.
pub(crate) fn full_to_half(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            '\u{3000}' => ' ', // 전각 공백 → 일반 공백.
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 반각(ASCII) 영숫자·기호·공백을 전각으로. 그 외 문자는 유지.
pub(crate) fn half_to_full(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            ' ' => '\u{3000}', // 일반 공백 → 전각 공백.
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_half_roundtrip() {
        assert_eq!(full_to_half("Ａ１　ｚ！"), "A1 z!");
        assert_eq!(half_to_full("A1 z!"), "Ａ１　ｚ！");
        // 한글/한자는 변환하지 않는다.
        assert_eq!(full_to_half("가Ａ"), "가A");
        assert_eq!(half_to_full("가A"), "가Ａ");
        for s in ["Hello 123!", "a-b_c"] {
            assert_eq!(full_to_half(&half_to_full(s)), s);
        }
    }
}
