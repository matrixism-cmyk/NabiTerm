//! **오래된 로그 정리** — 사용자 폴더가 조용히 부푸는 것을 막는다.
//!
//! 로그는 날마다 새 파일로 넘어가지만(`tracing_appender::rolling::daily`) **지워지지는
//! 않는다.** 매일 쓰면 1년에 365개가 쌓인다. 아무도 눈치채지 못한 채 늘어난다.
//!
//! ## 이 모듈은 사용자 디스크의 파일을 지운다
//!
//! 그래서 규칙을 코드로 못 박는다. 아래 판정은 전부 순수 함수이고, 시험은 **지우면 안 되는
//! 것을 지키는 쪽**을 더 두껍게 본다.
//!
//! * 지우는 것은 **우리가 만든 이름**뿐이다 — `nabi.log.YYYY-MM-DD`. 그 폴더에 사용자가
//!   둔 다른 파일은 건드리지 않는다.
//! * **오늘 파일은 지우지 않는다.** 지금 쓰고 있는 파일이다.
//! * 보관 일수가 0이면 **아무것도 하지 않는다**. 끄는 길은 반드시 있어야 한다.

/// 파일 이름이 우리 로그인가 — `nabi.log.YYYY-MM-DD`.
///
/// 날짜 모양까지 본다. `nabi.log.bak` 같은 것을 사용자가 만들어 뒀을 수 있다.
pub(crate) fn log_date(name: &str) -> Option<(i32, u32, u32)> {
    let rest = name.strip_prefix("nabi.log.")?;
    let mut it = rest.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// 날짜를 "며칠째"로 바꿔 견주기 쉽게 한다(정확한 달력이 아니라 순서만 필요하다).
fn ordinal(y: i32, m: u32, d: u32) -> i64 {
    y as i64 * 372 + m as i64 * 31 + d as i64
}

/// 지울 파일 이름들을 고른다.
///
/// `today`는 오늘 날짜, `keep_days`는 보관 일수(0이면 정리하지 않는다).
pub(crate) fn to_delete(names: &[String], today: (i32, u32, u32), keep_days: u32) -> Vec<String> {
    if keep_days == 0 {
        return Vec::new();
    }
    let cutoff = ordinal(today.0, today.1, today.2) - keep_days as i64;
    names
        .iter()
        .filter(|n| match log_date(n) {
            // 오늘 것과 미래 날짜는 건드리지 않는다(시계가 틀어졌을 수도 있다).
            Some((y, m, d)) => ordinal(y, m, d) < cutoff,
            None => false,
        })
        .cloned()
        .collect()
}

/// 폴더를 훑어 오래된 로그를 지운다. 지운 개수.
pub(crate) fn prune(dir: &std::path::Path, today: (i32, u32, u32), keep_days: u32) -> usize {
    if keep_days == 0 {
        return 0;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let names: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let mut n = 0;
    for name in to_delete(&names, today, keep_days) {
        if std::fs::remove_file(dir.join(&name)).is_ok() {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: (i32, u32, u32) = (2026, 8, 25);

    #[test]
    fn our_log_names_are_recognised() {
        assert_eq!(log_date("nabi.log.2026-08-25"), Some((2026, 8, 25)));
    }

    /// **우리 이름이 아닌 것은 건드리지 않는다.** 이 폴더에 사용자가 뭘 뒀는지 알 수 없다.
    #[test]
    fn anything_else_is_not_ours() {
        for n in [
            "nabi.log",
            "nabi.log.bak",
            "nabi.log.2026-08",
            "nabi.log.2026-08-25.old",
            "important.txt",
            "nabi.log.20260825",
            "",
        ] {
            assert!(log_date(n).is_none(), "우리 것이 아닌데 우리 것으로 봤다: {n}");
        }
    }

    /// 날짜 모양이지만 말이 안 되는 값은 거른다.
    #[test]
    fn impossible_dates_are_rejected() {
        assert!(log_date("nabi.log.2026-13-01").is_none());
        assert!(log_date("nabi.log.2026-00-01").is_none());
        assert!(log_date("nabi.log.2026-08-32").is_none());
    }

    #[test]
    fn old_files_are_selected() {
        let names = vec!["nabi.log.2026-06-01".to_string(), "nabi.log.2026-08-20".to_string()];
        let got = to_delete(&names, TODAY, 30);
        assert_eq!(got, vec!["nabi.log.2026-06-01".to_string()]);
    }

    /// **오늘 파일은 절대 지우지 않는다** — 지금 쓰고 있는 파일이다.
    #[test]
    fn todays_file_is_never_touched() {
        let names = vec!["nabi.log.2026-08-25".to_string()];
        assert!(to_delete(&names, TODAY, 1).is_empty());
        assert!(to_delete(&names, TODAY, 0).is_empty());
    }

    /// 시계가 틀어져 미래 날짜가 있어도 지우지 않는다.
    #[test]
    fn future_dates_are_left_alone() {
        let names = vec!["nabi.log.2027-01-01".to_string()];
        assert!(to_delete(&names, TODAY, 1).is_empty());
    }

    /// **끄는 길이 있어야 한다** — 0이면 아무것도 지우지 않는다.
    #[test]
    fn zero_means_never_prune() {
        let names = vec!["nabi.log.2000-01-01".to_string()];
        assert!(to_delete(&names, TODAY, 0).is_empty());
    }

    /// 우리 것이 아닌 파일이 섞여 있어도 그것만 남긴다.
    #[test]
    fn foreign_files_survive_a_prune() {
        let names = vec![
            "nabi.log.2020-01-01".to_string(),
            "my-notes.txt".to_string(),
            "nabi.log.bak".to_string(),
        ];
        assert_eq!(to_delete(&names, TODAY, 30), vec!["nabi.log.2020-01-01".to_string()]);
    }

    /// 실제 폴더에서도 우리 것만 사라져야 한다.
    #[test]
    fn a_real_folder_keeps_what_is_not_ours() {
        let d = std::env::temp_dir().join(format!("nabi-logprune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for n in ["nabi.log.2020-01-01", "nabi.log.2026-08-25", "keepme.txt"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        assert_eq!(prune(&d, TODAY, 30), 1);
        assert!(!d.join("nabi.log.2020-01-01").exists());
        assert!(d.join("nabi.log.2026-08-25").exists(), "오늘 것을 지웠다");
        assert!(d.join("keepme.txt").exists(), "남의 파일을 지웠다");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_folder_is_harmless() {
        assert_eq!(prune(std::path::Path::new("C:/no/such/folder/here"), TODAY, 30), 0);
    }
}
