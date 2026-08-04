//! 표준 base64 디코더(A-Za-z0-9+/ + `=` 패딩). 공백/개행은 무시. 외부 의존 없음.

/// base64 바이트열을 디코드한다. 알파벳 외 문자(공백·개행)는 건너뛴다. 실패 시 None.
pub fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for &b in input {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,            // 패딩 — 끝.
            b'\r' | b'\n' | b' ' | b'\t' => continue, // 공백 무시.
            _ => return None,         // 비정상 문자.
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::base64_decode;

    #[test]
    fn decodes_basic() {
        assert_eq!(base64_decode(b"aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode(b"Zm9v").unwrap(), b"foo");
        // 개행 섞여도 무시.
        assert_eq!(base64_decode(b"aGVs\nbG8=").unwrap(), b"hello");
    }
}
