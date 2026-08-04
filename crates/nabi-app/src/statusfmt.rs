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
    use super::{human_duration, short_path};

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
}
