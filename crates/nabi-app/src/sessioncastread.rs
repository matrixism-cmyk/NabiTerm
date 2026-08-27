//! `.cast` 되읽기 — 우리가 쓴 기록을 우리가 다시 읽는다(배치 Y S2 준비).
//!
//! 되읽기를 만드는 이유는 재생기를 위해서만이 아니다. **우리가 쓴 것을 우리가 못 읽으면
//! 남도 못 읽는다.** 쓰기만 만들고 끝내면 형식이 어긋나도 아무도 모른 채 파일이 쌓이고,
//! 나중에 재생하려는 순간에야 드러난다 — 그때는 이미 기록이 다 망가진 뒤다.
//!
//! 그래서 여기 있는 것은 왕복 시험을 위한 짝이기도 하다(`sessioncast`가 쓰고 이쪽이 읽는다).
//!
//! 화면에 다시 그릴 수 있는 것만 돌려준다 — 머리글 줄과 입력 이벤트(`"i"`)는 건너뛴다.

/// `.cast` 한 줄을 되읽는다 — `[경과초, "o", "내용"]` → `(경과초, 내용)`.
///
/// 출력이 아닌 줄(머리글·입력·깨진 줄)은 `None`. 깨진 줄에서 멈추지 않는 이유는, 기록이
/// 필요한 순간은 대개 무언가 잘못되고 있을 때이고 그때 끝이 잘린 파일이 흔하기 때문이다.
/// 마지막 한 줄이 잘렸다고 앞의 전부를 못 보게 되면 기록을 남긴 뜻이 없다.
pub(crate) fn parse_event(line: &str) -> Option<(f64, String)> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    let (secs, rest) = inner.split_once(',')?;
    let secs: f64 = secs.trim().parse().ok()?;
    // 두 번째 칸은 종류다. 출력("o")만 다시 그린다.
    let rest = rest.trim_start().strip_prefix('"')?;
    let (kind, rest) = rest.split_once('"')?;
    if kind != "o" {
        return None;
    }
    let rest = rest.trim_start().strip_prefix(',')?;
    Some((secs, unjson_string(rest)?))
}

/// 기록 전체를 읽어 출력 사건만 시간순으로 돌려준다.
pub(crate) fn parse_cast(text: &str) -> Vec<(f64, String)> {
    text.lines().filter_map(parse_event).collect()
}

/// 기록의 길이(초) — 마지막 사건의 경과초. 사건이 없으면 0.
pub(crate) fn duration(events: &[(f64, String)]) -> f64 {
    events.last().map(|(t, _)| *t).unwrap_or(0.0)
}

/// JSON 문자열 리터럴을 되푼다 — `sessioncast::json_string`의 반대.
fn unjson_string(s: &str) -> Option<String> {
    let body = s.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(body.len());
    let mut it = body.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'u' => {
                let hex: String = it.by_ref().take(4).collect();
                let n = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(n)?);
            }
            // 규격에 없는 벗어남은 조용히 삼키지 않는다 — 삼키면 내용이 몰래 바뀐다.
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessioncast::event;

    /// **왕복** — 우리가 쓴 것을 우리가 그대로 되읽는가. 이 시험이 이 파일의 존재 이유다.
    #[test]
    fn what_we_write_we_can_read_back() {
        let cases = [
            "hello",
            "\u{1b}[31mred\u{1b}[0m",       // 색상 — 터미널 출력의 대부분
            "안녕하세요 세계",                 // 한글
            "tab\there\r\nnext",             // 제어문자
            r#"quote " and backslash \ "#,   // JSON을 깨뜨리는 두 글자
            "\u{7f}\u{1}\u{1f}",             // 그 밖의 제어문자
            "",                              // 빈 출력
        ];
        for (i, text) in cases.iter().enumerate() {
            let line = event(1.25 + i as f64, text);
            let (t, back) = parse_event(&line).expect("되읽기");
            assert_eq!(&back, text, "내용이 왕복에서 변했다: {line}");
            assert!((t - (1.25 + i as f64)).abs() < 1e-9, "시각이 어긋났다");
        }
    }

    #[test]
    fn header_line_is_not_an_event() {
        // 첫 줄은 머리글이다. 이것을 사건으로 읽으면 재생 첫 순간에 쓰레기가 찍힌다.
        assert!(parse_event(r#"{"version":2,"width":80,"height":24}"#).is_none());
    }

    #[test]
    fn input_events_are_skipped() {
        // "i"는 사용자가 친 것이다. 다시 그리면 화면에 두 번 나온다.
        assert!(parse_event(r#"[1.0, "i", "ls"]"#).is_none());
        assert_eq!(parse_event(r#"[1.0, "o", "ls"]"#), Some((1.0, "ls".into())));
    }

    #[test]
    fn a_truncated_last_line_does_not_lose_the_rest() {
        // 기록이 필요한 순간은 대개 무언가 잘못되고 있을 때다. 끝이 잘린 파일이 흔하다.
        let text = format!(
            "{}\n{}\n{}\n[9.0, \"o\", \"unterminated",
            r#"{"version":2,"width":80,"height":24,"timestamp":0}"#,
            event(0.5, "first"),
            event(1.0, "second"),
        );
        let ev = parse_cast(&text);
        assert_eq!(ev.len(), 2, "앞의 두 줄은 살아야 한다");
        assert_eq!(ev[0].1, "first");
        assert!((duration(&ev) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_escape_is_rejected_not_swallowed() {
        // 삼키면 내용이 몰래 바뀐다 — 기록은 바뀌면 안 되는 것이다.
        assert!(parse_event(r#"[1.0, "o", "bad \q escape"]"#).is_none());
    }

    #[test]
    fn duration_of_nothing_is_zero() {
        assert_eq!(duration(&[]), 0.0);
    }
}
