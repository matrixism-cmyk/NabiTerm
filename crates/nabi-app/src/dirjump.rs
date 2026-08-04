//! zoxide식 디렉터리 점프(E1) — 방문 빈도+최근성(frecency)으로 자주 가는 폴더를 순위화.
//! cwd는 OSC7로 이미 추적됨(nabi-osc). 순수 함수 위주(단위테스트).

use std::collections::BTreeMap;

/// 방문 기록 = 디렉터리 → (방문 횟수, 마지막 방문 unix초).
pub(crate) type Visits = BTreeMap<String, (u32, i64)>;

/// frecency 점수: 횟수 × 최근성 가중(zoxide식 — 최근일수록 큰 가중).
pub(crate) fn frecency(count: u32, last: i64, now: i64) -> f64 {
    let age = (now - last).max(0);
    let w = if age < 3_600 {
        4.0
    } else if age < 86_400 {
        2.0
    } else if age < 604_800 {
        0.5
    } else {
        0.25
    };
    count as f64 * w
}

/// 디렉터리 방문을 기록한다(증가/삽입). 상한 초과 시 최저 frecency 항목을 제거.
pub(crate) fn record(map: &mut Visits, dir: &str, now: i64, cap: usize) {
    if dir.is_empty() {
        return;
    }
    let e = map.entry(dir.to_string()).or_insert((0, now));
    e.0 = e.0.saturating_add(1);
    e.1 = now;
    if map.len() > cap {
        if let Some(k) = map
            .iter()
            .min_by(|a, b| frecency(a.1 .0, a.1 .1, now).total_cmp(&frecency(b.1 .0, b.1 .1, now)))
            .map(|(k, _)| k.clone())
        {
            map.remove(&k);
        }
    }
}

/// frecency 내림차순 상위 n개 디렉터리.
pub(crate) fn top(map: &Visits, now: i64, n: usize) -> Vec<String> {
    let mut v: Vec<(&String, f64)> = map.iter().map(|(k, (c, l))| (k, frecency(*c, *l, now))).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v.into_iter().take(n).map(|(k, _)| k.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_beats_frequent_when_old() {
        let now = 1_000_000i64;
        // 오래된 고빈도(10회, 1주+전) vs 최근 저빈도(2회, 방금).
        let old = frecency(10, now - 700_000, now); // age>1주 → ×0.25 = 2.5
        let fresh = frecency(2, now, now); // ×4 = 8.0
        assert!(fresh > old);
    }

    #[test]
    fn record_and_top() {
        let now = 2_000_000i64;
        let mut m = Visits::new();
        record(&mut m, "/a", now, 100);
        record(&mut m, "/a", now, 100);
        record(&mut m, "/b", now, 100);
        assert_eq!(m["/a"].0, 2);
        assert_eq!(top(&m, now, 1), vec!["/a"]); // /a가 횟수 더 많아 상위
        record(&mut m, "", now, 100); // 빈 경로 무시
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn evicts_lowest_over_cap() {
        let now = 3_000_000i64;
        let mut m = Visits::new();
        record(&mut m, "/keep", now, 1); // cap=1
        record(&mut m, "/keep", now, 1);
        record(&mut m, "/new", now, 1); // 초과 → 최저(/new 또는 동률) 제거
        assert!(m.len() <= 1);
    }
}
