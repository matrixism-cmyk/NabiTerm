//! nabiPad 인코딩 도구 4탄(CyberChef 벤치마킹) — Quoted-Printable(MIME)·Base58(Bitcoin/IPFS).
//! 모두 순수 함수라 단위 테스트로 왕복 검증한다. "변환" 서브메뉴에 노출.

/// Quoted-Printable(RFC 2045) 인코딩. 인쇄 가능 ASCII는 그대로, 그 외(공백·`=`·고위 바이트)는 `=XX`.
/// 개행(`\n`)은 보존해 디코드가 정확히 왕복한다.
pub fn qp_encode(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for &b in t.as_bytes() {
        match b {
            b'\n' => out.push('\n'),
            33..=60 | 62..=126 => out.push(b as char), // `=`(61) 제외한 인쇄 ASCII.
            _ => out.push_str(&format!("={b:02X}")),
        }
    }
    out
}

/// Quoted-Printable 디코딩. 잘못된 시퀀스나 비UTF-8 결과면 원문을 유지한다.
pub fn qp_decode(t: &str) -> String {
    let b = t.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            if b.get(i + 1) == Some(&b'\n') {
                i += 2; // 소프트 줄바꿈 제거.
                continue;
            }
            match t.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(v) => {
                    out.push(v);
                    i += 3;
                    continue;
                }
                None => return t.to_string(), // 잘못된 `=` 시퀀스.
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| t.to_string())
}

const B58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Base58(Bitcoin 알파벳) 인코딩 — 입력 텍스트의 UTF-8 바이트를 부호화한다.
pub fn base58_encode(t: &str) -> String {
    let input = t.as_bytes();
    let zeros = input.iter().take_while(|&&b| b == 0).count();
    let mut digits: Vec<u8> = Vec::new(); // base58 자릿수(LSB first).
    for &byte in input {
        let mut carry = u32::from(byte);
        for d in &mut digits {
            carry += u32::from(*d) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(zeros + digits.len());
    for _ in 0..zeros {
        out.push('1'); // 선행 0 바이트 → '1'.
    }
    for &d in digits.iter().rev() {
        out.push(B58[d as usize] as char);
    }
    if out.is_empty() {
        String::new()
    } else {
        out
    }
}

/// Base58 디코딩 → 텍스트. 알파벳 밖 문자나 비UTF-8이면 원문 유지.
pub fn base58_decode(t: &str) -> String {
    let s = t.trim();
    let zeros = s.chars().take_while(|&c| c == '1').count();
    let mut bytes: Vec<u8> = Vec::new(); // base256(LSB first).
    for c in s.chars() {
        let Some(val) = B58.iter().position(|&x| x as char == c) else {
            return t.to_string();
        };
        let mut carry = val as u32;
        for b in &mut bytes {
            carry += u32::from(*b) * 58;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let mut out = vec![0u8; zeros];
    out.extend(bytes.iter().rev());
    String::from_utf8(out).unwrap_or_else(|_| t.to_string())
}

/// Ascii85(btoa/Adobe) 인코딩 — 4바이트→5문자(base85, '!'=33). 0 그룹은 'z'로 축약.
pub fn ascii85_encode(t: &str) -> String {
    let mut out = String::new();
    for chunk in t.as_bytes().chunks(4) {
        let mut buf = [0u8; 4];
        buf[..chunk.len()].copy_from_slice(chunk);
        let n = u32::from_be_bytes(buf);
        if chunk.len() == 4 && n == 0 {
            out.push('z');
            continue;
        }
        let mut digits = [0u8; 5];
        let mut v = n;
        for d in digits.iter_mut().rev() {
            *d = (v % 85) as u8;
            v /= 85;
        }
        for &d in &digits[..chunk.len() + 1] {
            out.push((d + 33) as char);
        }
    }
    out
}

/// Ascii85 디코딩 → 텍스트. 잘못된 문자나 비UTF-8이면 원문 유지.
pub fn ascii85_decode(t: &str) -> String {
    let mut bytes = Vec::new();
    let mut group: Vec<u32> = Vec::new();
    for c in t.chars() {
        if c.is_whitespace() {
            continue;
        }
        if c == 'z' && group.is_empty() {
            bytes.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !('!'..='u').contains(&c) {
            return t.to_string();
        }
        group.push(c as u32 - 33);
        if group.len() == 5 {
            let n = group.iter().fold(0u32, |a, &d| a.wrapping_mul(85).wrapping_add(d));
            bytes.extend_from_slice(&n.to_be_bytes());
            group.clear();
        }
    }
    if !group.is_empty() {
        let cnt = group.len();
        if cnt == 1 {
            return t.to_string(); // 끝에 한 글자만 남으면 무효.
        }
        while group.len() < 5 {
            group.push(84); // 'u'로 패딩.
        }
        let n = group.iter().fold(0u32, |a, &d| a.wrapping_mul(85).wrapping_add(d));
        bytes.extend_from_slice(&n.to_be_bytes()[..cnt - 1]);
    }
    String::from_utf8(bytes).unwrap_or_else(|_| t.to_string())
}

const B45: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// Base45(RFC 9285) 인코딩 — 2바이트→3문자(QR/EU DCC). 끝 1바이트는 2문자.
pub fn base45_encode(t: &str) -> String {
    let mut out = String::new();
    for ch in t.as_bytes().chunks(2) {
        if ch.len() == 2 {
            let n = u16::from(ch[0]) * 256 + u16::from(ch[1]);
            out.push(B45[(n % 45) as usize] as char);
            out.push(B45[(n / 45 % 45) as usize] as char);
            out.push(B45[(n / 45 / 45) as usize] as char);
        } else {
            let n = u16::from(ch[0]);
            out.push(B45[(n % 45) as usize] as char);
            out.push(B45[(n / 45) as usize] as char);
        }
    }
    out
}

/// Base45 디코딩 → 텍스트. 알파벳 밖 문자·과대값·비UTF-8이면 원문 유지.
pub fn base45_decode(t: &str) -> String {
    // 공백(' ')은 Base45 유효 문자이므로 제거하면 안 된다 — 개행만 무시한다.
    let chars: Vec<char> = t.chars().filter(|c| !matches!(c, '\n' | '\r')).collect();
    let mut bytes = Vec::new();
    for chunk in chars.chunks(3) {
        if chunk.len() == 1 {
            return t.to_string();
        }
        let mut v: u32 = 0;
        for &c in chunk.iter().rev() {
            match B45.iter().position(|&x| x as char == c) {
                Some(p) => v = v * 45 + p as u32,
                None => return t.to_string(),
            }
        }
        if chunk.len() == 3 {
            if v > 0xFFFF {
                return t.to_string();
            }
            bytes.push((v / 256) as u8);
            bytes.push((v % 256) as u8);
        } else {
            if v > 0xFF {
                return t.to_string();
            }
            bytes.push(v as u8);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base45_rfc_vectors() {
        assert_eq!(base45_encode("AB"), "BB8"); // RFC 9285 예시.
        assert_eq!(base45_encode("Hello!!"), "%69 VD92EX0");
        for s in ["", "AB", "Hello!!", "한글 ☕", "x"] {
            assert_eq!(base45_decode(&base45_encode(s)), s);
        }
        assert_eq!(base45_decode("!!!"), "!!!"); // 알파벳 밖 → 원문.
    }

    #[test]
    fn ascii85_roundtrip() {
        for s in ["", "Man", "sure.", "한글 ☕", "\0\0\0\0x"] {
            assert_eq!(ascii85_decode(&ascii85_encode(s)), s);
        }
        assert_eq!(ascii85_encode("\0\0\0\0"), "z"); // 0 그룹 축약.
        assert_eq!(ascii85_decode("!!!!!!"), "!!!!!!"); // 정상이지만 짧은 잔여는 위 케이스로 보장; 잘못된 문자 테스트:
        assert_eq!(ascii85_decode("~bad"), "~bad"); // 범위 밖 문자 → 원문.
    }

    #[test]
    fn qp_roundtrip() {
        assert_eq!(qp_encode("a=b c"), "a=3Db=20c");
        assert_eq!(qp_decode("a=3Db=20c"), "a=b c");
        for s in ["plain", "tab\there", "café ☕", "x=y&z=1\nnext"] {
            assert_eq!(qp_decode(&qp_encode(s)), s);
        }
        assert_eq!(qp_decode("=ZZ"), "=ZZ"); // 잘못된 시퀀스 보존.
    }

    #[test]
    fn base58_roundtrip() {
        assert_eq!(base58_encode("Hello World!"), "2NEpo7TZRRrLZSi2U");
        for s in ["", "abc", "한글 test", "\0\0x"] {
            assert_eq!(base58_decode(&base58_encode(s)), s);
        }
        assert_eq!(base58_decode("0OIl"), "0OIl"); // 알파벳 밖 문자 → 원문.
    }
}
