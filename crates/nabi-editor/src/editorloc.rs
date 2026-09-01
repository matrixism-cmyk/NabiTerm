//! AI 위치 복사용 줄 스펙 계산(순수, 테스트). 선택이 여러 줄이면 "시작-끝", 아니면 단일 줄(1-base).
//! AI 바이브코딩: 선택 영역을 `경로:10-25`처럼 가리켜 LLM이 정확한 줄 범위를 인식하게 한다.

/// 선택 범위(문자 인덱스)가 여러 줄이면 "시작-끝", 같은 줄/선택 없음이면 단일 줄(1-base) 문자열.
pub fn loc_linespec(text: &str, range: Option<(usize, usize)>, cur_line: usize) -> String {
    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(i, _)| i);
    match range.filter(|(a, b)| a != b) {
        Some((a, b)) => {
            let (lo, hi) = (a.min(b), a.max(b));
            let sl = text[..byte(lo)].matches('\n').count() + 1;
            let el = text[..byte(hi)].matches('\n').count() + 1;
            linespec(sl, el)
        }
        None => (cur_line + 1).to_string(),
    }
}

/// 줄 번호 둘(1-base)을 스펙 글자로. 같은 줄이면 하나만 적는다.
///
/// 작은 문서와 대용량 rope 편집기가 **이 함수를 함께 쓴다.** 형식을 두 벌로 두면
/// 한쪽만 고쳐지는 날 같은 파일이 창마다 다르게 적힌다.
pub fn linespec(start_line: usize, end_line: usize) -> String {
    let (a, b) = (start_line.min(end_line), start_line.max(end_line));
    if a == b {
        a.to_string()
    } else {
        format!("{a}-{b}")
    }
}

/// 바이트 오프셋 둘을 스펙 글자로 — HEX 편집기의 "여기를 보라"다.
///
/// 글에 줄이 있듯 바이트에는 오프셋이 있다. 형식은 화면 상태바와 같은 `0x` + 여덟 자리
/// 대문자 16진이다(`edithexview` 가 `0x{cur:08X}` 로 적는다) — 화면에서 본 숫자와 복사한
/// 숫자가 다르면 사람이 둘을 잇지 못한다.
///
/// 한 바이트면 하나만, 범위면 `시작-끝`. **`linespec` 과 나란히 둔 이유**는 같다:
/// 위치를 적는 방법이 두 곳에 흩어지면 한쪽만 고쳐지는 날이 온다.
pub fn offsetspec(start: usize, end: usize) -> String {
    let (a, b) = (start.min(end), start.max(end));
    if a == b {
        format!("0x{a:08X}")
    } else {
        format!("0x{a:08X}-0x{b:08X}")
    }
}

/// 줄 이동 입력 "줄[:열]"(1-base)을 (줄0, 열?)로 파싱(VS Code식 Go to Line:Col). 잘못되면 None.
pub fn parse_goto(s: &str) -> Option<(usize, Option<u32>)> {
    let t = s.trim();
    let (lp, col) = t.split_once(':').map_or((t, None), |(a, b)| (a, b.trim().parse::<u32>().ok()));
    let line = lp.trim().parse::<usize>().ok()?;
    Some((line.saturating_sub(1), col))
}

#[cfg(test)]
mod tests {
    use super::{loc_linespec, parse_goto};

    #[test]
    fn linespec_cases() {
        let t = "a\nbb\nccc\nd"; // 줄: a(1) bb(2) ccc(3) d(4)
        assert_eq!(loc_linespec(t, None, 2), "3"); // 선택 없음 → 커서 줄(0-base 2 → 3).
        assert_eq!(loc_linespec(t, Some((2, 7)), 0), "2-3"); // bb~ccc 걸침 → 2~3줄.
        assert_eq!(loc_linespec(t, Some((5, 7)), 0), "3"); // 같은 줄 → 단일.
        assert_eq!(loc_linespec(t, Some((4, 4)), 1), "2"); // 빈 선택(a==b) → 커서 줄.
    }

    #[test]
    fn goto_cases() {
        assert_eq!(parse_goto("42"), Some((41, None))); // 줄만(1→0-base).
        assert_eq!(parse_goto(" 42 : 5 "), Some((41, Some(5)))); // 줄:열 + 공백.
        assert_eq!(parse_goto("7:"), Some((6, None))); // 열 비면 줄만.
        assert_eq!(parse_goto("x"), None); // 줄 아님.
    }

    /// **작은 문서와 대용량 편집기가 같은 답을 내야 한다.**
    ///
    /// 두 창이 같은 파일의 같은 자리를 다르게 적으면, AI 에게 넘긴 위치가 어느 쪽 것인지
    /// 알 수 없다. 형식을 한 함수에 모은 이유가 이것이다.
    #[test]
    fn both_editors_write_the_same_location() {
        use super::linespec;
        // 한 줄이면 번호 하나, 여러 줄이면 범위.
        assert_eq!(linespec(7, 7), "7");
        assert_eq!(linespec(3, 9), "3-9");
        // 거꾸로 골라도 같은 답이다(아래에서 위로 드래그).
        assert_eq!(linespec(9, 3), "3-9");
    }

    /// **HEX 위치도 화면에 적힌 것과 같은 모양이어야 한다.**
    ///
    /// 상태바는 `0x0000001A` 로 적는다. 복사한 것이 `26` 이나 `0x1a` 면, 받아 본 사람도
    /// 적은 사람도 그것이 같은 자리인지 알 수 없다.
    #[test]
    fn the_hex_offset_looks_like_the_status_bar() {
        use super::offsetspec;
        assert_eq!(offsetspec(0x1A, 0x1A), "0x0000001A", "한 바이트면 하나만");
        assert_eq!(offsetspec(0x1A, 0x2F), "0x0000001A-0x0000002F");
        // 거꾸로 골라도 같은 답(아래에서 위로 끌었을 때).
        assert_eq!(offsetspec(0x2F, 0x1A), "0x0000001A-0x0000002F");
    }

    /// 글로 세는 쪽(`loc_linespec`)도 같은 형식을 쓴다.
    #[test]
    fn the_string_path_uses_the_same_format() {
        let text = "a\nb\nc\nd\n";
        // "b\nc" 를 고른 것 — 2~3 줄.
        assert_eq!(loc_linespec(text, Some((2, 5)), 0), "2-3");
        assert_eq!(loc_linespec(text, Some((2, 3)), 0), "2", "한 줄이면 번호 하나");
        assert_eq!(loc_linespec(text, None, 4), "5", "선택이 없으면 커서 줄");
    }
}
