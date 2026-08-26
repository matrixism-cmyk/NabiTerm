//! **걸린 시간을 사람이 읽는 꼴로.**
//!
//! 명령 목록에 밀리초를 그대로 늘어놓으면 `1847293ms`가 된다. 눈으로 크기를 가늠할 수
//! 없는 숫자다. 자릿수를 줄이고 단위를 바꿔 **한눈에 큰지 작은지** 알게 한다.
//!
//! 반올림해서 `0s`가 되는 것은 피한다 — 끝난 명령을 "0초 걸렸다"고 하면 재지 못한 것과
//! 구별되지 않는다. 1초 미만은 밀리초로 남긴다.

/// 밀리초를 짧은 글자로. 붙는 단위는 언어와 무관한 기호(ms/s/m/h)라 옮길 것이 없다.
pub fn human_ms(ms: u64) -> String {
    match ms {
        // 1초 미만은 밀리초 그대로 — 여기서 반올림하면 "0s"가 되어 못 잰 것처럼 보인다.
        0..=999 => format!("{ms}ms"),
        // 10초 미만은 소수 한 자리(2.4s) — 이 구간은 차이가 눈에 들어온다.
        1_000..=9_999 => format!("{}.{}s", ms / 1000, (ms % 1000) / 100),
        10_000..=59_999 => format!("{}s", ms / 1000),
        60_000..=3_599_999 => {
            let (m, s) = (ms / 60_000, (ms % 60_000) / 1000);
            format!("{m}m {s}s")
        }
        3_600_000..=86_399_999 => {
            let (h, m) = (ms / 3_600_000, (ms % 3_600_000) / 60_000);
            format!("{h}h {m}m")
        }
        _ => {
            // 하루가 넘는 명령도 있다(감시·빌드 서버). 다만 자릿수가 끝없이 늘면 목록이
            // 깨지므로 999일에서 끊고 **끊었다고 말한다** — 숫자를 잘라 속이지 않는다.
            let d = ms / 86_400_000;
            match d {
                0..=999 => format!("{d}d {}h", (ms % 86_400_000) / 3_600_000),
                _ => "999d+".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::human_ms;

    /// **1초 미만이 `0s`가 되면 안 된다** — 못 잰 것과 구별할 수 없어진다.
    #[test]
    fn a_fast_command_keeps_its_milliseconds() {
        assert_eq!(human_ms(0), "0ms");
        assert_eq!(human_ms(7), "7ms");
        assert_eq!(human_ms(999), "999ms");
    }

    /// 초 단위 구간은 소수 한 자리까지 — 이 구간은 차이가 눈에 들어온다.
    #[test]
    fn a_few_seconds_shows_one_decimal() {
        assert_eq!(human_ms(1_000), "1.0s");
        assert_eq!(human_ms(2_450), "2.4s");
        assert_eq!(human_ms(9_999), "9.9s");
    }

    #[test]
    fn ten_seconds_and_up_drops_the_decimal() {
        assert_eq!(human_ms(10_000), "10s");
        assert_eq!(human_ms(59_999), "59s");
    }

    #[test]
    fn minutes_and_hours_read_as_two_parts() {
        assert_eq!(human_ms(60_000), "1m 0s");
        assert_eq!(human_ms(3_599_999), "59m 59s");
        assert_eq!(human_ms(3_600_000), "1h 0m");
        assert_eq!(human_ms(7_530_000), "2h 5m");
    }

    /// 하루가 넘어도 읽힌다(감시·빌드 서버는 그런 명령을 돌린다).
    #[test]
    fn days_are_shown_and_capped_honestly() {
        assert_eq!(human_ms(86_400_000), "1d 0h");
        assert_eq!(human_ms(90_000_000), "1d 1h");
        // 끝없이 늘어나는 대신 끊고, **끊었다고 말한다**.
        assert_eq!(human_ms(u64::MAX / 2), "999d+");
    }

    /// 어떤 값이 와도 짧아야 한다 — 목록 한 줄에 들어가야 하기 때문이다.
    #[test]
    fn every_answer_stays_short() {
        for ms in [0, 1, 999, 1_000, 59_999, 3_599_999, u64::MAX / 2] {
            assert!(human_ms(ms).len() <= 8, "{ms} -> {}", human_ms(ms));
        }
    }
}
