//! 상태바 표시 포맷 — 경로 줄임·소요시간 사람이 읽는 형태. 순수 함수라 테스트가 붙어 있다.

/// 긴 경로를 "…/끝에서두번째/마지막"으로 축약한다(세그먼트 2개 이하면 그대로).
pub(crate) fn short_path(p: &str) -> String {
    let parts: Vec<&str> = p.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    if parts.len() <= 2 {
        p.to_string()
    } else {
        format!("\u{2026}/{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
    }
}

/// 초를 사람이 읽는 길이로 — **이 규칙은 여기 하나뿐이다**(배치 AD).
///
/// 같은 일을 하는 함수가 다섯 개 있었고, **답이 서로 달랐다.** 3661초를 두고
/// `1h01m`(AI 상태·전송 ETA) · `1h 01m`(명령 이력) · `1h 1m`(접속 이력) 세 가지가 나왔다.
/// 사용자에게는 다 같은 "얼마나 걸렸나"인데 화면마다 다르게 보인 것이다.
///
/// 모양은 가장 꼼꼼했던 것(명령 이력)을 따른다 — **자리를 채우고 사이를 띄운다.**
/// `1h 4m` 처럼 자리를 안 채우면 목록에서 숫자가 들쭉날쭉해 눈으로 훑기 어렵다.
pub(crate) fn human_secs(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60),
    }
}

/// 명령 실행 시간을 사람친화 단위(ms/s/m/h)로 포맷한다.
pub(crate) fn human_duration(ms: u128) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let secs = ms / 1000;
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        let mins = ms / 60_000;
        format!("{}h {}m", mins / 60, mins % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::{human_duration, human_secs, short_path};

    #[test]
    fn duration_units() {
        assert_eq!(human_duration(250), "250ms");
        assert_eq!(human_duration(1500), "1.5s");
        assert_eq!(human_duration(90_000), "1m 30s");
        assert_eq!(human_duration(5_400_000), "1h 30m");
    }

    #[test]
    fn short_path_keeps_tail() {
        assert_eq!(short_path("C:/a/b/c/d"), "\u{2026}/c/d");
        assert_eq!(short_path("C:/only"), "C:/only");
        assert_eq!(short_path("/home"), "/home");
    }
    #[test]
    fn seconds_under_a_minute_stay_seconds() {
        assert_eq!(human_secs(0), "0s");
        assert_eq!(human_secs(45), "45s");
    }

    #[test]
    fn minutes_and_hours_pad_and_space() {
        // 자리를 안 채우면 목록에서 숫자가 들쭉날쭉해 눈으로 훑기 어렵다.
        assert_eq!(human_secs(60), "1m 00s");
        assert_eq!(human_secs(187), "3m 07s");
        assert_eq!(human_secs(3600), "1h 00m");
        assert_eq!(human_secs(3661), "1h 01m");
    }

    #[test]
    fn every_screen_shows_the_same_duration_the_same_way() {
        // 이 시험이 이 함수가 생긴 이유다. 예전에는 같은 3661초가 화면마다 달랐다:
        // AI 상태·전송 ETA 는 "1h01m", 명령 이력은 "1h 01m", 접속 이력은 "1h 1m".
        let one = human_secs(3661);
        assert_eq!(crate::cmdhist::human_secs(3661), one);
        assert_eq!(crate::connhist::human_secs(3661), one);
        assert_eq!(crate::sftpxfer::human_secs(3661), one);
    }

}
