//! 키워드 하이라이트 규칙 파싱 — "단어" 또는 "단어=#RRGGBB"(로그 모니터링).

use nabi_types::Rgba;

/// "단어" 또는 "단어=#RRGGBB"를 (단어, 색)으로 나눈다. 색이 없거나 형식 오류면 기본색.
pub(crate) fn split_highlight_rule(s: &str, default: Rgba) -> (&str, Rgba) {
    match s.split_once('=') {
        Some((word, col)) => (word.trim(), Rgba::from_hex(col.trim()).unwrap_or(default)),
        None => (s, default),
    }
}

#[cfg(test)]
mod tests {
    use super::split_highlight_rule;
    use nabi_types::Rgba;

    #[test]
    fn parses_optional_color() {
        let def = Rgba::rgb(1, 2, 3);
        assert_eq!(split_highlight_rule("ERROR", def), ("ERROR", def)); // 색 없음 → 기본.
        assert_eq!(
            split_highlight_rule("WARN=#ff8800", def),
            ("WARN", Rgba::rgb(0xff, 0x88, 0x00))
        );
        assert_eq!(split_highlight_rule("X=zzz", def), ("X", def)); // 잘못된 색 → 기본.
    }
}
