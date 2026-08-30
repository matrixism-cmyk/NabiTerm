//! 사람이 읽는 표기 헬퍼 — 크기/날짜/상대시간/요약/브레드크럼(browserfs에서 분리).

use std::path::{Path, PathBuf};

/// 로컬 경로를 브레드크럼으로 분해: (표시 라벨, 그 지점까지의 경로).
/// 드라이브("C:")는 루트 경로("C:\")로 맞춘다. / 와 \ 둘 다 구분자로 본다.
pub fn local_crumbs(path: &Path) -> Vec<(String, PathBuf)> {
    let full = path.to_string_lossy().into_owned();
    let mut out = Vec::new();
    let mut acc = String::new();
    for seg in full.split(['/', '\\']).filter(|s| !s.is_empty()) {
        if !acc.is_empty() {
            acc.push(std::path::MAIN_SEPARATOR);
        }
        acc.push_str(seg);
        let target = if acc.ends_with(':') {
            format!("{acc}{}", std::path::MAIN_SEPARATOR)
        } else {
            acc.clone()
        };
        out.push((seg.to_string(), PathBuf::from(target)));
    }
    out
}

/// 항목들에서 (폴더 수, 파일 수, 파일 총 바이트)를 센다(디렉터리 크기는 합산 안 함).
pub fn summarize(items: impl Iterator<Item = (bool, u64)>) -> (usize, usize, u64) {
    let (mut dirs, mut files, mut bytes) = (0usize, 0usize, 0u64);
    for (is_dir, size) in items {
        if is_dir {
            dirs += 1;
        } else {
            files += 1;
            bytes += size;
        }
    }
    (dirs, files, bytes)
}

/// 바이트 수를 사람이 읽는 단위로(B/KB/MB/GB/TB).
pub fn human(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let f = n as f64;
    if n < 1024 {
        format!("{n} B")
    } else if f < MB {
        format!("{:.1} KB", f / KB)
    } else if f < GB {
        format!("{:.1} MB", f / MB)
    } else if f < TB {
        format!("{:.1} GB", f / GB)
    } else {
        format!("{:.1} TB", f / TB)
    }
}

/// unix 초를 YYYY-MM-DD로(0이면 빈 문자열). Hinnant 민용력 알고리즘(외부 의존 없음).
pub fn human_date(secs: u64) -> String {
    // 현지 시간대로 센다. 예전에는 여기만 UTC 로 셈해서, **같은 파일이 같은 목록 안에서
    // 다른 날짜로 보였다** — 나이 칸은 `human_date`(UTC), 수정일시 칸은 `human_datetime`
    // (현지)를 쓰기 때문이다. 한국(+9)에서는 새벽에 고친 파일이 하루 전으로 보였다.
    //
    // 손으로 셈하던 것(civil-from-days)을 그대로 두지 않고 `human_datetime` 과 **같은
    // 길**로 보낸다. 셈이 둘이면 언젠가 또 갈린다.
    let dt = human_datetime(secs);
    match dt.split_once(' ') {
        Some((date, _)) => date.to_string(),
        None => dt, // 0 이거나 시간대 변환이 실패한 경우 — 그대로(빈 글).
    }
}

/// unix 초를 로컬 시간대 "YYYY-MM-DD HH:MM"으로(0이면 빈 문자열). 탐색기 수정일시 컬럼용.
pub fn human_datetime(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(secs as i64, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => String::new(),
    }
}

/// 수정 시각(unix초)을 상대 표기로: 최근은 Nm/Nh/Nd, 일주일 넘으면 절대 날짜. 0이면 "".
pub fn human_age(mtime: u64, now: u64) -> String {
    if mtime == 0 {
        return String::new();
    }
    let age = now.saturating_sub(mtime);
    if age < 3600 {
        format!("{}m", age / 60)
    } else if age < 86400 {
        format!("{}h", age / 3600)
    } else if age < 7 * 86400 {
        format!("{}d", age / 86400)
    } else {
        human_date(mtime)
    }
}

/// 앞에서 n자까지만(넘치면 말줄임) — 목록 셀 표시용.
pub fn ellipsis(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}\u{2026}", s.chars().take(n).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::{human, human_age, human_date, human_datetime, local_crumbs, summarize};
    use std::path::Path;

    #[test]
    fn size_scales() {
        assert_eq!(human(512), "512 B");
        assert!(human(2048).contains("KB"));
        assert!(human(5 * 1024 * 1024).contains("MB"));
    }

    #[test]
    fn date_and_age() {
        assert_eq!(human_date(0), "");
        assert!(human_date(1_000_000).contains('-'));
        assert_eq!(human_age(1000, 1000 + 120), "2m");
        assert_eq!(human_age(1000, 1000 + 3 * 86400), "3d");
        assert!(human_age(1000, 1000 + 9 * 86400).contains('-'));
    }

    #[test]
    fn datetime_format() {
        assert_eq!(human_datetime(0), ""); // 0 → 빈 문자열.
        let s = human_datetime(1_000_000_000);
        assert_eq!(s.len(), 16); // "YYYY-MM-DD HH:MM" (타임존 무관 길이).
        assert!(s.contains('-') && s.contains(':'));
    }

    #[test]
    fn summarize_and_crumbs() {
        let items = [(true, 0u64), (false, 100), (false, 50), (true, 0)];
        assert_eq!(summarize(items.into_iter()), (2, 2, 150));
        let c = local_crumbs(Path::new("C:\\Users\\u"));
        assert_eq!(
            c.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>(),
            vec!["C:", "Users", "u"]
        );
    }
}


#[cfg(test)]
mod date_agree_tests {
    use super::{human_date, human_datetime};

    /// **한 목록 안에서 같은 파일이 다른 날짜로 보이면 안 된다.**
    ///
    /// 나이 칸(`human_age` → `human_date`)과 수정일시 칸(`human_datetime`)이 같은 값을
    /// 받아 서로 다른 날을 말하던 적이 있다 — 한쪽은 UTC, 한쪽은 현지였다.
    /// 한국(+9)에서는 새벽에 고친 파일이 하루 전으로 보였다.
    #[test]
    fn 날짜와_일시가_같은_날을_말한다() {
        for secs in [1u64, 86_399, 86_400, 1_700_000_000, 1_767_225_600, 2_000_000_000] {
            let d = human_date(secs);
            let dt = human_datetime(secs);
            assert_eq!(d, dt.split(' ').next().unwrap_or(""), "{secs} 에서 갈렸다");
        }
    }

    #[test]
    fn 시각이_없으면_빈_글() {
        assert_eq!(human_date(0), "");
        assert_eq!(human_datetime(0), "");
    }
}
