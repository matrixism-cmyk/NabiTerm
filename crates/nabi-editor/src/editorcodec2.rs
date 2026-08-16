//! nabiPad 추가 코덱(CyberChef 벤치마킹) — Base64URL·Binary·Decimal·ROT47. 모두 순수 함수.

use crate::editorconvert::{base64_decode, base64_encode};

/// URL-safe Base64 인코딩(`+/`→`-_`, 패딩 제거). JWT/웹에서 흔함.
pub fn base64url_encode(t: &str) -> String {
    base64_encode(t).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

/// URL-safe Base64 디코딩(패딩 복원 후 표준 디코드). 잘못된 입력이면 원문 유지.
pub fn base64url_decode(t: &str) -> String {
    let mut s = t.trim().replace('-', "+").replace('_', "/");
    while !s.len().is_multiple_of(4) {
        s.push('=');
    }
    base64_decode(&s)
}

/// 텍스트 → 8비트 2진(바이트별, 공백 구분).
pub fn to_binary(t: &str) -> String {
    t.as_bytes().iter().map(|b| format!("{b:08b}")).collect::<Vec<_>>().join(" ")
}

/// 2진(0/1만 추려 8비트씩) → 텍스트. 비트 수가 8의 배수가 아니거나 비UTF-8이면 원문 유지.
pub fn from_binary(t: &str) -> String {
    let bits: Vec<u8> = t.bytes().filter(|&b| b == b'0' || b == b'1').map(|b| b - b'0').collect();
    if bits.is_empty() || !bits.len().is_multiple_of(8) {
        return t.to_string();
    }
    let bytes: Vec<u8> = bits.chunks(8).map(|c| c.iter().fold(0u8, |a, &x| (a << 1) | x)).collect();
    String::from_utf8(bytes).unwrap_or_else(|_| t.to_string())
}

/// 텍스트 → 10진 바이트값(공백 구분).
pub fn to_decimal(t: &str) -> String {
    t.as_bytes().iter().map(u8::to_string).collect::<Vec<_>>().join(" ")
}

/// 10진 숫자열(공백/쉼표 구분, 0~255) → 텍스트. 범위 밖·비UTF-8이면 원문 유지.
pub fn from_decimal(t: &str) -> String {
    let mut bytes = Vec::new();
    for tok in t.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()) {
        match tok.parse::<u8>() {
            Ok(b) => bytes.push(b),
            Err(_) => return t.to_string(),
        }
    }
    if bytes.is_empty() {
        return t.to_string();
    }
    String::from_utf8(bytes).unwrap_or_else(|_| t.to_string())
}

/// ROT47(인쇄 가능한 ASCII 33~126을 47 회전). 두 번 적용하면 원문.
pub fn rot47(t: &str) -> String {
    t.chars()
        .map(|c| {
            let u = c as u32;
            if (33..=126).contains(&u) {
                char::from_u32(33 + (u - 33 + 47) % 94).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// ROT5 — 숫자 0~9만 5 회전(그 외 유지). 두 번 적용하면 원문. ROT13의 숫자판.
pub fn rot5(t: &str) -> String {
    t.chars().map(|c| if c.is_ascii_digit() { (((c as u8 - b'0' + 5) % 10) + b'0') as char } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rot5_digits() {
        assert_eq!(rot5("abc123"), "abc678");
        assert_eq!(rot5(&rot5("Year 2026!")), "Year 2026!");
    }

    #[test]
    fn base64url_roundtrip() {
        for s in ["", "hi", "🦋?>~ data", "사용자"] {
            assert_eq!(base64url_decode(&base64url_encode(s)), s);
        }
        // 표준 Base64의 +/ 가 -_ 로 바뀌고 패딩이 없다.
        let e = base64url_encode("🦋");
        assert!(!e.contains('+') && !e.contains('/') && !e.contains('='));
    }

    #[test]
    fn binary_and_decimal() {
        assert_eq!(to_binary("A"), "01000001");
        assert_eq!(from_binary("01000001 01000010"), "AB");
        assert_eq!(from_binary("nope"), "nope"); // 비트 없음→원문.
        assert_eq!(to_decimal("AB"), "65 66");
        assert_eq!(from_decimal("72, 105"), "Hi"); // 쉼표 허용.
        assert_eq!(from_decimal("999"), "999"); // 255 초과→원문.
    }

    #[test]
    fn rot47_roundtrip() {
        assert_eq!(rot47("Hello, World!"), "w6==@[ (@C=5P");
        assert_eq!(rot47(&rot47("nabiTerm @#%")), "nabiTerm @#%");
    }
}
