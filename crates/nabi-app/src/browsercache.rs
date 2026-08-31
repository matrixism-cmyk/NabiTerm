//! **폴더를 매 프레임 다시 읽지 않는다** — 목록 캐시의 갱신 규칙.
//!
//! ## 무엇이 잘못돼 있었나
//!
//! 파일 브라우저는 그리는 함수 안에서 `read_entries` 를 불렀다. 그리기는 초당 예순 번
//! 일어나므로, 폴더가 바뀌지 않아도 초당 예순 번 읽었다. 항목마다 `metadata()` 를
//! 부르니 시스템 호출은 그 곱만큼 난다.
//!
//! 재 봤다(2026-09-01): 항목 2,000개 폴더 한 번 읽기 **1.1ms**, 60fps 면 **초당 64ms** —
//! 아무 일도 없는 폴더를 보고만 있어도 코어 하나의 6%를 쓴다. 로컬 SSD 에서 그렇다.
//! 네트워크 경로(UNC·폐쇄망 공유)에서는 항목 하나가 곧 왕복이라 훨씬 나쁘다.
//!
//! ## 왜 시간으로 끊는가
//!
//! "바뀔 때만 다시 읽기"가 이상적이지만, 그러려면 **바꾸는 자리를 하나도 빠짐없이** 알아야
//! 한다. 붙여넣기·삭제·이름변경·압축풀기·새 폴더… 게다가 밖에서 다른 프로그램이 바꾸는
//! 것은 우리가 알 길이 없다. 하나라도 놓치면 목록이 낡은 채로 남고, 그건 느린 것보다 나쁘다.
//!
//! 그래서 **짧은 간격으로 다시 읽는다.** 놓친 변화도 그 간격 안에 저절로 낫는다.
//! 0.4초면 사람 눈에는 즉시이고, 읽는 횟수는 초당 예순에서 둘 반으로 줄어든다(96% 감소).
//! 우리가 직접 바꾼 직후에는 기다리지 않고 바로 다시 읽는다.

use std::time::{Duration, Instant};

/// 다시 읽기까지 기다리는 시간.
///
/// 밖에서 파일이 생기거나 사라진 것을 이만큼 늦게 안다. 더 짧으면 이득이 줄고, 더 길면
/// 사람이 "왜 안 보이지" 하고 새로고침을 찾게 된다.
pub(crate) const REFRESH_EVERY: Duration = Duration::from_millis(400);

/// 목록 캐시가 무엇을 담고 있는지 — 이것이 달라지면 내용도 다르다.
///
/// 정렬 기준까지 넣는 까닭은 `read_entries` 가 정렬까지 해서 돌려주기 때문이다.
/// 정렬만 바꿔도 결과가 달라지므로 같은 폴더라도 다시 읽어야 한다.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CacheKey {
    pub path: std::path::PathBuf,
    pub sort: crate::browserfs::Sort,
    pub desc: bool,
    pub hidden: bool,
}

/// 지금 다시 읽어야 하는가.
///
/// * `key` 가 달라졌으면(폴더를 옮겼거나 정렬을 바꿨으면) 곧바로.
/// * 우리가 방금 뭔가를 바꿨으면(`dirty`) 곧바로 — 기다리게 하면 지운 파일이 남아 보인다.
/// * 그 밖에는 마지막으로 읽은 지 `REFRESH_EVERY` 가 지났을 때.
pub(crate) fn needs_reread(
    have: Option<&CacheKey>,
    want: &CacheKey,
    last: Option<Instant>,
    dirty: bool,
    now: Instant,
) -> bool {
    if dirty || have != Some(want) {
        return true;
    }
    match last {
        None => true, // 읽은 적이 없다.
        Some(t) => now.duration_since(t) >= REFRESH_EVERY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browserfs::Sort;

    fn key(p: &str) -> CacheKey {
        CacheKey { path: p.into(), sort: Sort::Name, desc: false, hidden: false }
    }

    #[test]
    fn the_first_look_always_reads() {
        assert!(needs_reread(None, &key("a"), None, false, Instant::now()));
    }

    #[test]
    fn the_same_folder_within_the_interval_is_not_reread() {
        let now = Instant::now();
        let k = key("a");
        assert!(!needs_reread(Some(&k), &k, Some(now), false, now));
    }

    #[test]
    fn after_the_interval_it_reads_again() {
        let now = Instant::now();
        let k = key("a");
        let old = now - REFRESH_EVERY - Duration::from_millis(1);
        assert!(needs_reread(Some(&k), &k, Some(old), false, now));
    }

    #[test]
    fn moving_to_another_folder_reads_at_once() {
        let now = Instant::now();
        assert!(needs_reread(Some(&key("a")), &key("b"), Some(now), false, now));
    }

    /// 같은 폴더라도 **정렬이 바뀌면** 내용이 달라진다 — 캐시를 그대로 쓰면 안 된다.
    #[test]
    fn changing_the_sort_also_reads_at_once() {
        let now = Instant::now();
        let a = key("a");
        let mut b = a.clone();
        b.desc = true;
        assert!(needs_reread(Some(&a), &b, Some(now), false, now));
        let mut c = a.clone();
        c.sort = Sort::Size;
        assert!(needs_reread(Some(&a), &c, Some(now), false, now));
        let mut d = a.clone();
        d.hidden = true;
        assert!(needs_reread(Some(&a), &d, Some(now), false, now), "숨김 표시도 목록을 바꾼다");
    }

    /// 우리가 방금 바꿨으면 기다리지 않는다 — 지운 파일이 잠깐이라도 남아 보이면 안 된다.
    #[test]
    fn right_after_we_changed_something_it_reads_at_once() {
        let now = Instant::now();
        let k = key("a");
        assert!(needs_reread(Some(&k), &k, Some(now), true, now));
    }
}
