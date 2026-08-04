//! nabiPad 변환/인코딩 도구(ROT13·Base32·Text↔Hex·유니코드 이스케이프)와 "변환" 서브메뉴.
//! CyberChef/개발도구 벤치마킹. 모두 순수 함수라 단위 테스트로 검증한다.

use crate::edithexedit::{parse_hex, to_hex_string};
use nabi_i18n::{tr, Lang};

/// ROT13(영문 알파벳만 13 회전, 그 외 유지). 두 번 적용하면 원문.
pub(crate) fn rot13(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            _ => c,
        })
        .collect()
}

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 Base32 인코딩(패딩 `=` 포함).
pub(crate) fn base32_encode(t: &str) -> String {
    let mut out = String::new();
    for chunk in t.as_bytes().chunks(5) {
        let mut buf = [0u8; 5];
        buf[..chunk.len()].copy_from_slice(chunk);
        let n = u64::from(buf[0]) << 32 | u64::from(buf[1]) << 24 | u64::from(buf[2]) << 16 | u64::from(buf[3]) << 8 | u64::from(buf[4]);
        let chars = [0, 2, 4, 5, 7, 8][chunk.len()]; // 1→2,2→4,3→5,4→7,5→8 출력문자.
        for i in 0..8 {
            if i < chars {
                out.push(B32[((n >> (35 - i * 5)) & 0x1f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// RFC 4648 Base32 디코딩 → UTF-8. 잘못된 입력이면 원문 유지.
pub(crate) fn base32_decode(t: &str) -> String {
    let (mut bits, mut nbits, mut out) = (0u64, 0u32, Vec::new());
    for c in t.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=') {
        let v = match c.to_ascii_uppercase() {
            b'A'..=b'Z' => u64::from(c.to_ascii_uppercase() - b'A'),
            b'2'..=b'7' => u64::from(c - b'2') + 26,
            _ => return t.to_string(),
        };
        bits = (bits << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| t.to_string())
}

/// 텍스트 → 공백 구분 16진("Hi"→"48 69"). edithexedit 재사용.
pub(crate) fn hex_text_encode(t: &str) -> String {
    to_hex_string(t.as_bytes())
}

/// 16진 문자열 → 텍스트(공백 등 무시). 비UTF-8이면 원문 유지.
pub(crate) fn hex_text_decode(t: &str) -> String {
    String::from_utf8(parse_hex(t)).unwrap_or_else(|_| t.to_string())
}

/// 비ASCII 문자를 JSON식 `\uXXXX`로(astral은 서로게이트 쌍). ASCII는 그대로.
pub(crate) fn unicode_escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        let u = c as u32;
        if u < 0x80 {
            out.push(c);
        } else if u <= 0xFFFF {
            out.push_str(&format!("\\u{u:04X}"));
        } else {
            let v = u - 0x10000;
            out.push_str(&format!("\\u{:04X}\\u{:04X}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF)));
        }
    }
    out
}

/// `\uXXXX`(서로게이트 쌍 포함)를 문자로. 잘못된 시퀀스는 그대로 둔다.
pub(crate) fn unicode_unescape(t: &str) -> String {
    let b = t.as_bytes();
    let mut out = String::with_capacity(t.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && b.get(i + 1) == Some(&b'u') {
            if let Some(hi) = t.get(i + 2..i + 6).and_then(|s| u32::from_str_radix(s, 16).ok()) {
                if (0xD800..0xDC00).contains(&hi) && t.get(i + 6..i + 8) == Some("\\u") {
                    if let Some(lo) = t.get(i + 8..i + 12).and_then(|s| u32::from_str_radix(s, 16).ok()) {
                        if let Some(ch) = char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)) {
                            out.push(ch);
                            i += 12;
                            continue;
                        }
                    }
                }
                if let Some(ch) = char::from_u32(hi) {
                    out.push(ch);
                    i += 6;
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

/// 텍스트를 JSON 문자열 리터럴로(따옴표 감싸고 이스케이프). 소스/JSON에 박을 때.
pub(crate) fn json_string_escape(t: &str) -> String {
    serde_json::to_string(t).unwrap_or_else(|_| t.to_string())
}

/// JSON 문자열 리터럴 → 원문(파싱). 잘못된 입력이면 원문 유지.
pub(crate) fn json_string_unescape(t: &str) -> String {
    serde_json::from_str::<String>(t.trim()).unwrap_or_else(|_| t.to_string())
}

use crate::editmenugroups::{pick, Xf};

/// "변환" 서브메뉴 — 인코딩/이스케이프/포맷 하위 그룹으로 분류(2단계 계층, CyberChef식).
pub(crate) fn convert_menu(ui: &mut egui::Ui, lang: Lang) -> Option<Xf> {
    use crate::editorcodec2 as c2;
    use crate::editorcodec4 as c4;
    use crate::editorconvert as cv;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.encgroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.b64enc", cv::base64_encode), ("editor.b64dec", cv::base64_decode),
            ("editor.b64urlenc", c2::base64url_encode), ("editor.b64urldec", c2::base64url_decode),
            ("editor.base32enc", base32_encode), ("editor.base32dec", base32_decode),
            ("editor.base58enc", c4::base58_encode), ("editor.base58dec", c4::base58_decode),
            ("editor.a85enc", c4::ascii85_encode), ("editor.a85dec", c4::ascii85_decode),
            ("editor.b45enc", c4::base45_encode), ("editor.b45dec", c4::base45_decode),
            ("editor.hexenc", hex_text_encode), ("editor.hexdec", hex_text_decode),
            ("editor.tobin", c2::to_binary), ("editor.frombin", c2::from_binary),
            ("editor.todec", c2::to_decimal), ("editor.fromdec", c2::from_decimal),
            ("editor.urlenc", cv::url_encode), ("editor.urldec", cv::url_decode),
            ("editor.qpenc", c4::qp_encode), ("editor.qpdec", c4::qp_decode),
        ]));
    });
    ui.menu_button(tr(lang, "editor.escgroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.escape", cv::backslash_escape), ("editor.unescape", cv::backslash_unescape),
            ("editor.htmlenc", cv::html_encode), ("editor.htmldec", cv::html_decode),
            ("editor.uniesc", unicode_escape), ("editor.uniunesc", unicode_unescape),
            ("editor.jsonstresc", json_string_escape), ("editor.jsonstrunesc", json_string_unescape),
            ("editor.shellquote", cv::shell_quote),
        ]));
    });
    ui.menu_button(tr(lang, "editor.fmtgroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.jsonpretty", cv::json_pretty), ("editor.jsonmin", cv::json_minify),
            ("editor.xmlpretty", crate::editorxml::xml_pretty), ("editor.xmlmin", crate::editorxml::xml_minify),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rot13_roundtrip() {
        assert_eq!(rot13("Hello, Zorld!"), "Uryyb, Mbeyq!");
        assert_eq!(rot13(&rot13("nabiTerm 123")), "nabiTerm 123");
    }

    #[test]
    fn base32_rfc4648_vectors() {
        assert_eq!(base32_encode("foobar"), "MZXW6YTBOI======");
        assert_eq!(base32_encode("f"), "MY======");
        for s in ["", "f", "fo", "foobar", "한글"] {
            assert_eq!(base32_decode(&base32_encode(s)), s);
        }
        assert_eq!(base32_decode("!!!"), "!!!"); // 잘못된 입력→원문.
    }

    #[test]
    fn hex_text_roundtrip() {
        assert_eq!(hex_text_encode("Hi"), "48 69");
        assert_eq!(hex_text_decode("48 69"), "Hi");
        assert_eq!(hex_text_decode("48656c6c6f"), "Hello"); // 공백 없어도.
    }

    #[test]
    fn json_string_escape_roundtrip() {
        assert_eq!(json_string_escape("a\"b\nc"), "\"a\\\"b\\nc\"");
        assert_eq!(json_string_unescape("\"a\\\"b\\nc\""), "a\"b\nc");
    }

    #[test]
    fn unicode_escape_roundtrip() {
        assert_eq!(unicode_escape("aé"), "a\\u00E9");
        assert_eq!(unicode_escape("🦋"), "\\uD83E\\uDD8B"); // astral=서로게이트 쌍.
        for s in ["plain", "café", "한글 🦋 test"] {
            assert_eq!(unicode_unescape(&unicode_escape(s)), s);
        }
        assert_eq!(unicode_unescape("\\uZZZZ"), "\\uZZZZ"); // 잘못된 시퀀스 보존.
    }
}
