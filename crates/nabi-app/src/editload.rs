//! 파일 바이트 → 텍스트 디코드 + 인코딩/EOL 자동 감지.
//! BOM 우선 → chardetng 추정 → encoding_rs 디코드. 라벨·EOL을 함께 돌려준다.

use encoding_rs::Encoding;

/// 바이트를 (텍스트, 인코딩 라벨, EOL)로 디코드한다.
pub(crate) fn decode(bytes: &[u8]) -> (String, String, &'static str) {
    let enc = detect_encoding(bytes);
    // encoding_rs는 BOM이 있으면 그 인코딩을 우선 적용한다(actual로 반환).
    let (text, actual, _had_err) = enc.decode(bytes);
    let label = actual.name().to_string();
    let eol = detect_eol(&text);
    (text.into_owned(), label, eol)
}

/// 인코딩 추정: BOM 우선, 없으면 chardetng(순수 Rust)로 추정(UTF-8 허용).
pub(crate) fn detect_encoding(bytes: &[u8]) -> &'static Encoding {
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
pub(crate) fn decode_with(bytes: &[u8], label: &str) -> (String, String, &'static str) {
    let enc = Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (text, actual, _) = enc.decode(bytes);
    let eol = detect_eol(&text);
    (text.into_owned(), actual.name().to_string(), eol)
}

/// 텍스트를 지정 인코딩 라벨로 인코딩한다(저장용). 미상 라벨은 UTF-8. 매핑 불가 문자는
/// encoding_rs 규칙(HTML 수치 참조 등)으로 대체된다.
pub(crate) fn encode(text: &str, label: &str) -> Vec<u8> {
    let enc = Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    enc.encode(text).0.into_owned()
}

/// 줄 끝 추정(CRLF/LF/CR).
pub(crate) fn detect_eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "CRLF"
    } else if text.contains('\r') {
        "CR"
    } else {
        "LF"
    }
}
