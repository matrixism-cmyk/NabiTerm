//! 원격 파일 **미리보기** — 내려받지 않고 앞부분만 본다.
//!
//! "이 파일이 내가 찾던 그 설정 파일인가"를 확인하려고 매번 내려받는 것은 낭비다. 특히
//! 느린 회선에서는 확인 한 번이 몇 분이 된다. 앞 몇 KB만 읽으면 대부분 판가름난다.
//!
//! ## 왜 상한이 중요한가
//!
//! 원격 파일은 크기를 믿을 수 없다(심볼릭 링크, /proc 같은 가짜 파일, 잘못된 stat).
//! 그래서 **크기를 묻지 않고 처음부터 상한만큼만 읽는다.** 몇 GB짜리를 실수로 끌어올
//! 길을 아예 만들지 않는다.
//!
//! ## 무엇을 재사용하는가
//!
//! 이진 판정과 인코딩 추정은 편집기가 이미 잘하고 있다(`nabi_editor::edithex::is_binary`,
//! `editload::detect_encoding`). 여기서 다시 만들면 두 곳의 판정이 갈라진다 — 같은 파일이
//! 편집기에서는 텍스트, 미리보기에서는 이진으로 보이는 식이다.

/// 앞부분을 읽어 본 결과.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Preview {
    /// 글로 읽힌다.
    Text {
        body: String,
        /// 어떤 인코딩으로 읽었는지(화면에 적어 준다 — 깨져 보이면 이게 실마리다).
        encoding: String,
        /// 파일이 더 있는데 잘랐는가.
        truncated: bool,
    },
    /// 글이 아니다 — 앞부분을 16진으로 보여 준다.
    Binary { hex: String, shown: usize },
    /// 빈 파일.
    Empty,
}

/// 한 줄에 보여 줄 16진 바이트 수.
const HEX_COLS: usize = 16;
/// 16진으로 보여 줄 최대 바이트(이진 파일은 앞 몇 줄이면 정체가 드러난다).
const HEX_MAX: usize = 256;

/// 읽어 온 바이트를 화면에 낼 모양으로 바꾼다. `more`는 뒤에 내용이 더 있는지.
pub(crate) fn describe(bytes: &[u8], more: bool) -> Preview {
    if bytes.is_empty() {
        return Preview::Empty;
    }
    if nabi_editor::edithex::is_binary(bytes) {
        let n = bytes.len().min(HEX_MAX);
        return Preview::Binary { hex: hex_dump(&bytes[..n]), shown: n };
    }
    let enc = nabi_editor::editload::detect_encoding(bytes);
    let (text, _, _) = enc.decode(bytes);
    // 마지막 줄은 잘린 중간일 수 있다 — 더 있으면 그 줄을 버려 반 토막 글자를 안 보여 준다.
    let body = match more {
        true => drop_last_line(&text),
        false => text.into_owned(),
    };
    Preview::Text { body, encoding: enc.name().to_string(), truncated: more }
}

/// 마지막 줄을 버린다(잘린 자리라 온전하지 않다). 줄이 하나뿐이면 그대로 둔다.
fn drop_last_line(text: &str) -> String {
    match text.rfind('\n') {
        Some(i) => text[..i].to_string(),
        None => text.to_string(),
    }
}

/// 오프셋 + 16진 + 아스키 — 흔한 헥스 덤프 모양.
fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(HEX_COLS).enumerate() {
        out.push_str(&format!("{:08x}  ", row * HEX_COLS));
        for b in chunk {
            out.push_str(&format!("{b:02x} "));
        }
        for _ in chunk.len()..HEX_COLS {
            out.push_str("   ");
        }
        out.push(' ');
        for b in chunk {
            out.push(match *b {
                0x20..=0x7e => *b as char,
                _ => '.',
            });
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_says_so() {
        assert_eq!(describe(b"", false), Preview::Empty);
    }

    #[test]
    fn plain_text_is_shown_as_text() {
        let p = describe(b"hello\nworld\n", false);
        match p {
            Preview::Text { body, truncated, .. } => {
                assert!(body.contains("hello"));
                assert!(!truncated);
            }
            other => panic!("글인데 {other:?}로 봤다"),
        }
    }

    /// **잘린 마지막 줄은 버린다** — 반 토막 글자가 화면에 남으면 깨진 파일로 오해한다.
    #[test]
    fn a_truncated_tail_line_is_dropped() {
        let p = describe("첫 줄\n둘째 줄\n셋째 줄이 잘린".as_bytes(), true);
        match p {
            Preview::Text { body, truncated, .. } => {
                assert!(truncated);
                assert!(body.contains("둘째 줄"));
                assert!(!body.contains("셋째"), "잘린 줄이 남았다: {body:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// 줄바꿈이 하나도 없으면 버릴 것이 없다 — 통째로 사라지면 안 된다.
    #[test]
    fn a_single_long_line_is_not_thrown_away() {
        let p = describe(b"one very long line without any newline", true);
        match p {
            Preview::Text { body, .. } => assert!(body.starts_with("one very long")),
            other => panic!("{other:?}"),
        }
    }

    /// 이진 파일은 16진으로 — 글로 우기면 화면이 제어문자로 엉망이 된다.
    #[test]
    fn binary_content_is_shown_as_hex() {
        let mut b = vec![0u8; 40];
        b[0] = 0x7f;
        b[1] = b'E';
        match describe(&b, false) {
            Preview::Binary { hex, shown } => {
                assert_eq!(shown, 40);
                assert!(hex.contains("00000000"), "오프셋이 없다: {hex}");
                assert!(hex.contains("7f 45"), "{hex}");
            }
            other => panic!("이진인데 {other:?}"),
        }
    }

    /// 이진 미리보기도 상한이 있다 — 몇 MB를 16진으로 펴면 창이 멈춘다.
    #[test]
    fn the_hex_view_is_bounded() {
        let big = vec![0u8; 10_000];
        match describe(&big, true) {
            Preview::Binary { shown, .. } => assert_eq!(shown, HEX_MAX),
            other => panic!("{other:?}"),
        }
    }

    /// **UTF-16 텍스트를 이진으로 보면 안 된다** — 편집기가 겪은 회귀와 같은 함정.
    #[test]
    fn utf16_text_is_still_text() {
        let mut b = vec![0xFF, 0xFE];
        for c in "hello".encode_utf16() {
            b.extend_from_slice(&c.to_le_bytes());
        }
        match describe(&b, false) {
            Preview::Text { body, encoding, .. } => {
                assert!(body.contains("hello"), "{body:?}");
                assert!(encoding.contains("UTF-16"), "{encoding}");
            }
            other => panic!("UTF-16을 {other:?}로 봤다"),
        }
    }

    /// 한글(CP949)도 읽히고, 어떤 인코딩으로 읽었는지 말해 준다.
    #[test]
    fn legacy_korean_text_is_decoded_and_labelled() {
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("한글 설정 파일입니다. 나비텀에서 미리 봅니다.");
        match describe(&bytes, false) {
            Preview::Text { body, encoding, .. } => {
                assert!(body.contains("한글"), "{body:?}");
                assert!(!encoding.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    /// 헥스 덤프 한 줄은 오프셋·16진·아스키가 모두 있어야 읽을 값이 있다.
    #[test]
    fn a_hex_row_has_offset_bytes_and_ascii() {
        let dump = hex_dump(b"AB");
        assert!(dump.starts_with("00000000  "));
        assert!(dump.contains("41 42"));
        assert!(dump.trim_end().ends_with("AB"), "{dump:?}");
    }
}
