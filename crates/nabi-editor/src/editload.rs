//! 파일 바이트 → 텍스트 디코드 + 인코딩/EOL 자동 감지.
//! BOM 우선 → chardetng 추정 → encoding_rs 디코드. 라벨·EOL을 함께 돌려준다.

use encoding_rs::Encoding;

/// 바이트를 (텍스트, 인코딩 라벨, EOL)로 디코드한다.
pub fn decode(bytes: &[u8]) -> (String, String, &'static str) {
    let enc = detect_encoding(bytes);
    // encoding_rs는 BOM이 있으면 그 인코딩을 우선 적용한다(actual로 반환).
    let (text, actual, _had_err) = enc.decode(bytes);
    let label = actual.name().to_string();
    let eol = detect_eol(&text);
    (text.into_owned(), label, eol)
}

/// 인코딩 추정: BOM 우선, 없으면 chardetng(순수 Rust)로 추정(UTF-8 허용).
pub fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
    if let Some(enc) = bom_encoding(bytes) {
        return enc;
    }
    let mut det = chardetng::EncodingDetector::new();
    // 큰 파일은 앞부분 표본만 먹여도 충분(속도).
    let sample = &bytes[..bytes.len().min(64 * 1024)];
    det.feed(sample, true);
    det.guess(None, true)
}

/// BOM으로 인코딩 식별(UTF-8/16 LE·BE).
fn bom_encoding(b: &[u8]) -> Option<&'static Encoding> {
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(encoding_rs::UTF_8)
    } else if b.starts_with(&[0xFF, 0xFE]) {
        Some(encoding_rs::UTF_16LE)
    } else if b.starts_with(&[0xFE, 0xFF]) {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    }
}

/// 주어진 라벨로 디코드(인코딩 수동 재지정 — 재로드용).
pub fn decode_with(bytes: &[u8], label: &str) -> (String, String, &'static str) {
    let enc = Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (text, actual, _) = enc.decode(bytes);
    let eol = detect_eol(&text);
    (text.into_owned(), actual.name().to_string(), eol)
}

/// 텍스트를 지정 인코딩 라벨로 인코딩한다(저장용). 미상 라벨은 UTF-8. 매핑 불가 문자는
/// encoding_rs 규칙(HTML 수치 참조 등)으로 대체된다.
pub fn encode(text: &str, label: &str) -> Vec<u8> {
    let enc = Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    enc.encode(text).0.into_owned()
}

/// 줄 끝 추정 — **규칙은 [`crate::eolmix`] 한 곳에만 있다**(배치 AE).
///
/// 예전에는 여기서 "어디든 CRLF 가 하나라도 있으면 CRLF" 라고 봤고, 초대용량 경로
/// (`textdata`)는 "첫 개행이 정한다" 였다. 같은 파일에 대해 **답이 달랐다**:
/// LF 로 시작해 중간에 CRLF 가 한 번 섞인 파일을 이쪽은 CRLF, 저쪽은 LF 로 읽는다.
/// 그러면 같은 파일을 어느 편집기로 여느냐에 따라 Enter 가 **다른 줄바꿈을 넣는다** —
/// 파일 내용이 달라지는 차이다.
///
/// 이제 셋 다 `count_eols` + `dominant()` 를 쓴다. 동률 규칙(CRLF → LF → CR)도 거기 있다.
pub fn detect_eol(text: &str) -> &'static str {
    crate::eolmix::count_eols(text).dominant()
}

/// 줄 끝을 LF 하나로 맞춘다 — **한 번만 훑고, 필요 없으면 사본을 만들지 않는다.**
///
/// `replace("\r\n","\n").replace('\r',"\n")`는 문자열을 두 번 새로 만든다. 큰 파일에서는
/// 그 두 벌이 원본·디코드본과 함께 살아 있어 메모리 피크를 배로 올린다. 유닉스 줄 끝
/// 파일(대부분)은 `\r`이 아예 없으므로 받은 문자열을 그대로 돌려준다.
pub fn normalize_lf(s: String) -> String {
    if !s.as_bytes().contains(&b'\r') {
        return s; // 사본 없음.
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            // CRLF는 둘을 합쳐 하나로, 홀로 선 CR도 LF로.
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod lf_tests {
    use super::normalize_lf;

    #[test]
    fn crlf_and_lone_cr_become_lf() {
        assert_eq!(normalize_lf("a\r\nb\rc\nd".into()), "a\nb\nc\nd");
        assert_eq!(normalize_lf("\r\n\r\n".into()), "\n\n");
        assert_eq!(normalize_lf("\r".into()), "\n");
    }

    /// `\r`이 없으면 **같은 버퍼를 그대로 돌려준다** — 큰 파일에서 사본 한 벌이 사라진다.
    #[test]
    fn text_without_cr_is_returned_untouched() {
        let s = "한 줄\n두 줄\n".to_string();
        let ptr = s.as_ptr();
        let out = normalize_lf(s);
        assert_eq!(out, "한 줄\n두 줄\n");
        assert_eq!(out.as_ptr(), ptr, "사본을 만들면 안 된다");
    }

    #[test]
    fn multibyte_text_survives() {
        assert_eq!(normalize_lf("가나\r\n다라\r마바".into()), "가나\n다라\n마바");
    }
}
