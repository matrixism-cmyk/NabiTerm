//! nabiPad 텍스트 변환 도구 — JSON 포맷/압축, Base64·URL 인코딩/디코딩.
//! 모두 전체 문서 대상 순수 함수 `fn(&str)->String`. 실패(잘못된 입력)면 원문을 유지해
//! 데이터를 잃지 않는다. Edit ▸ 변환 메뉴의 xform 체인에 그대로 얹힌다.

/// JSON을 2칸 들여쓰기로 정렬. 파싱 실패면 원문 유지.
pub fn json_pretty(t: &str) -> String {
    serde_json::from_str::<serde_json::Value>(t)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| t.to_string())
}

/// JSON을 한 줄로 압축. 파싱 실패면 원문 유지.
pub fn json_minify(t: &str) -> String {
    serde_json::from_str::<serde_json::Value>(t)
        .ok()
        .and_then(|v| serde_json::to_string(&v).ok())
        .unwrap_or_else(|| t.to_string())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 표준 Base64 인코딩(전체 문서 바이트, 패딩 `=` 포함).
pub fn base64_encode(t: &str) -> String {
    base64_encode_bytes(t.as_bytes())
}

/// 임의 바이트열의 표준 Base64 인코딩(HEX 선택 복사 등 공용).
pub fn base64_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(B64[(n >> 18 & 63) as usize] as char);
        out.push(B64[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { B64[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { B64[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Base64 → UTF-8 텍스트. 비base64/비UTF-8이면 원문 유지.
pub fn base64_decode(t: &str) -> String {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    }
    let clean: Vec<u8> = t.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for c in clean.chunks(4) {
        if c.len() < 2 {
            return t.to_string();
        }
        let mut n = 0u32;
        for (i, &b) in c.iter().enumerate() {
            match val(b) {
                Some(v) => n |= v << (18 - 6 * i),
                None => return t.to_string(),
            }
        }
        out.push((n >> 16) as u8);
        if c.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if c.len() > 3 {
            out.push(n as u8);
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| t.to_string())
}

/// URL 퍼센트 인코딩(비예약 문자 `A-Za-z0-9-_.~` 외 모두 %XX). 전체 문서.
pub fn url_encode(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for &b in t.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// URL 퍼센트 디코딩. 잘못된 %시퀀스/비UTF-8이면 원문 유지.
pub fn url_decode(t: &str) -> String {
    let bytes = t.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return t.to_string();
            }
            match ((bytes[i + 1] as char).to_digit(16), (bytes[i + 2] as char).to_digit(16)) {
                (Some(h), Some(l)) => {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                }
                _ => return t.to_string(),
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| t.to_string())
}

/// 텍스트를 한 줄 문자열 리터럴로 — 역슬래시·개행·탭·CR·따옴표 이스케이프.
pub fn backslash_escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len() + 8);
    for c in t.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

/// POSIX 셸 안전 인용(작은따옴표 래핑, 내부 '는 `'\''`로). 원격/로컬 명령 인자 조립용(SSH 도구).
pub use nabi_proto::shquote::shell_quote;

/// 역슬래시 이스케이프 해제(\\ \n \t \r \0 \", 그 외 `\c`는 c).
pub fn backslash_unescape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut it = t.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some(other) => out.push(other), // \\ \" 및 알 수 없는 \c → c.
            None => out.push('\\'),
        }
    }
    out
}

/// HTML 특수문자 → 엔티티(& < > " ').
pub fn html_encode(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// HTML 엔티티 → 문자(명명 5종 + 숫자 `&#NN;` / `&#xHH;`). 모르는 건 그대로.
pub fn html_decode(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if t.as_bytes()[i] == b'&' {
            if let Some(semi) = t[i..].find(';') {
                if let Some(c) = decode_entity(&t[i + 1..i + semi]) {
                    out.push(c);
                    i += semi + 1;
                    continue;
                }
            }
        }
        let c = t[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn decode_entity(ent: &str) -> Option<char> {
    match ent {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = ent.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_padding() {
        assert_eq!(base64_encode("Man"), "TWFu");
        assert_eq!(base64_encode("Ma"), "TWE=");
        assert_eq!(base64_encode("M"), "TQ==");
    }

    #[test]
    fn base64_roundtrip() {
        for s in ["", "a", "hello world", "한글 텍스트 🦋"] {
            assert_eq!(base64_decode(&base64_encode(s)), s);
        }
        assert_eq!(base64_decode("not valid !!!"), "not valid !!!"); // 실패 시 원문.
    }

    #[test]
    fn url_roundtrip() {
        assert_eq!(url_encode("a b/c?d=1"), "a%20b%2Fc%3Fd%3D1");
        assert_eq!(url_decode("a%20b%2Fc"), "a b/c");
        assert_eq!(url_decode("bad%2"), "bad%2"); // 잘린 시퀀스→원문.
    }

    #[test]
    fn json_format() {
        assert!(json_pretty("{\"a\":1}").contains('\n'));
        assert_eq!(json_minify("{\n  \"a\": 1\n}"), "{\"a\":1}");
        assert_eq!(json_pretty("not json"), "not json"); // 실패 시 원문.
    }

    #[test]
    fn escape_roundtrip() {
        let raw = "a\nb\tc\"d\\e\r";
        assert_eq!(backslash_escape(raw), "a\\nb\\tc\\\"d\\\\e\\r");
        assert_eq!(backslash_unescape(&backslash_escape(raw)), raw);
        assert_eq!(shell_quote("it's ok"), "'it'\\''s ok'"); // POSIX 안전 인용.
    }

    #[test]
    fn html_entities() {
        assert_eq!(html_encode("<a href=\"x\">&'"), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;");
        assert_eq!(html_decode(&html_encode("<a>&\"'")), "<a>&\"'");
        assert_eq!(html_decode("&#65;/&#x41;"), "A/A");
        assert_eq!(html_decode("plain & raw"), "plain & raw"); // 비엔티티 &는 그대로.
    }
}
