//! 세션 로그에 **시각**을 담는다(배치 Y S1) — asciinema v2 `.cast` 형식.
//!
//! `sessionlog.rs`는 이미 터미널 출력을 파일로 남긴다. 없던 것은 시각이다:
//!
//! ```text
//! let _ = writeln!(log.file, "{joined}");   // 줄만 적었다
//! ```
//!
//! 무엇이 있었는지는 남지만 **어떤 속도로 흘렀는지는 잃는다.** 장애를 재현할 때 정작
//! 중요한 것이 시간 간격인데 그것이 없어 되감을 수 없었다.
//!
//! ## 왜 우리 형식을 만들지 않는가
//!
//! asciinema v2는 사실상 표준이고, 웹 플레이어·CLI·GitHub 임베드가 이미 읽는다. 우리 형식을
//! 새로 만들면 **우리 것으로만** 볼 수 있다. 기록은 남기는 것이 목적이 아니라 나중에 남에게
//! 보여 주는 것이 목적이다.
//!
//! 형식은 한 줄에 하나씩인 JSON이다 — 첫 줄이 머리글, 그다음은 `[경과초, "o", "내용"]`.
//! 줄 단위라 도중에 프로그램이 죽어도 **그때까지가 그대로 유효한 파일**이다. 이 점이
//! 터미널 기록에 특히 맞는다(기록이 필요한 순간은 대개 무언가 잘못되고 있을 때다).

/// `.cast` 첫 줄 — 판 번호와 터미널 크기, 시작 시각(유닉스 초).
///
/// 크기를 적는 이유: 재생기가 이 크기로 화면을 잡아야 줄바꿈이 원본과 같아진다.
pub(crate) fn header(cols: u16, rows: u16, unix_secs: u64) -> String {
    format!(r#"{{"version":2,"width":{cols},"height":{rows},"timestamp":{unix_secs}}}"#)
}

/// 출력 한 덩어리 — `[경과초, "o", "내용"]`.
///
/// 경과초는 소수점 여섯 자리로 적는다(asciinema 관례). 정수로 줄이면 빠르게 지나간
/// 출력이 한 순간에 몰려 재생이 실제와 달라진다.
pub(crate) fn event(elapsed: f64, text: &str) -> String {
    format!("[{:.6}, \"o\", {}]", elapsed.max(0.0), json_string(text))
}

/// JSON 문자열 리터럴로 감싼다 — 따옴표·역슬래시·제어문자를 규격대로 벗어난다.
///
/// 직접 쓰는 이유: 이 크레이트에 JSON 인코더가 없고, 여기서 필요한 것은 문자열 하나뿐이다.
/// 다만 **제어문자를 빠뜨리면 파일 전체가 깨진다** — 터미널 출력에는 ESC(0x1b)가 가득하고,
/// 그것이 규격대로 벗어나지 않으면 재생기가 첫 줄에서 멈춘다. 그래서 시험을 붙였다.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // 그 밖의 제어문자(ESC 포함)는 \u00XX로. 터미널 출력에는 이것이 가장 많다.
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_has_the_shape_players_expect() {
        assert_eq!(
            header(120, 30, 1_700_000_000),
            r#"{"version":2,"width":120,"height":30,"timestamp":1700000000}"#
        );
    }

    #[test]
    fn event_keeps_sub_second_timing() {
        // 정수로 줄이면 빠르게 지나간 출력이 한 순간에 몰려 재생이 실제와 달라진다.
        assert_eq!(event(1.5, "hi"), "[1.500000, \"o\", \"hi\"]");
        assert_eq!(event(0.000123, "x"), "[0.000123, \"o\", \"x\"]");
    }

    #[test]
    fn negative_elapsed_is_clamped() {
        // 시계가 뒤로 갈 일은 없어야 하지만, 음수가 나가면 재생기가 파일을 거부한다.
        assert_eq!(event(-3.0, "x"), "[0.000000, \"o\", \"x\"]");
    }

    #[test]
    fn escape_sequences_survive() {
        // 터미널 출력의 대부분이 ESC로 시작한다. 이것이 새면 파일 전체가 못 읽힌다.
        assert_eq!(json_string("\u{1b}[31mred"), r#""\u001b[31mred""#);
    }

    #[test]
    fn quotes_and_backslashes_are_escaped() {
        assert_eq!(json_string(r#"say "hi"\"#), r#""say \"hi\"\\""#);
    }

    #[test]
    fn newlines_and_tabs_use_short_forms() {
        assert_eq!(json_string("a\r\nb\tc"), r#""a\r\nb\tc""#);
    }

    #[test]
    fn hangul_is_written_as_is() {
        // UTF-8 그대로 둔다 — \u 로 바꾸면 파일이 커지고 사람이 읽기 어려워진다.
        assert_eq!(json_string("안녕"), "\"안녕\"");
    }

    #[test]
    fn delete_char_is_escaped_too() {
        assert_eq!(json_string("\u{7f}"), r#""\u007f""#);
    }
}
