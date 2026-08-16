//! 인코딩 깨짐 감지 순수 로직 — U+FFFD 비율과 대안 제안(앱 encsuggest가 재수출).

/// 텍스트 중 U+FFFD(치환문자) 비율 0..1. 빈 문자열이면 0.
pub fn replacement_ratio(text: &str) -> f32 {
    let (mut total, mut bad) = (0u32, 0u32);
    for c in text.chars() {
        total += 1;
        if c == '\u{fffd}' {
            bad += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        bad as f32 / total as f32
    }
}

pub fn suggest_alt(current_label: &str, ratio: f32) -> Option<&'static str> {
    if ratio < 0.02 {
        return None;
    }
    let cur = current_label.to_ascii_uppercase().replace(['-', '_'], "");
    match cur.as_str() {
        "UTF8" => Some("EUC-KR"),
        // 그 외(EUC-KR/CP949/Shift_JIS 등)가 깨지면 UTF-8 시도가 가장 흔한 정답.
        _ => Some("UTF-8"),
    }
}
