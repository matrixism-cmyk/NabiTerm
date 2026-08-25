//! **심볼릭 링크 따라가기** — 링크가 가리키는 것이 폴더인지 알아낸다.
//!
//! `readdir`가 주는 속성은 링크 **자신**의 것이라 종류가 늘 `Symlink`다. 그래서 지금까지는
//! 서버 쪽에서 흔한 `/data -> /mnt/vol1/data` 같은 폴더 링크를 더블클릭해도 들어가지 못했다
//! (우클릭 "링크로 들어가기"를 알고 있어야 했다).
//!
//! 대상 종류는 따라가는 `stat` 한 번으로 알 수 있다. 문제는 **횟수**다.
//!
//! ## 왕복을 어떻게 아끼는가
//!
//! * 링크가 아닌 항목은 건드리지 않는다.
//! * 한 목록에서 최대 [`MAX_RESOLVE`]개까지만 확인한다. 링크가 수백 개인 폴더에서
//!   목록이 열리기까지 몇 초씩 걸리는 것보다, 몇 개가 링크 그대로 남는 편이 낫다.
//! * 확인은 [`BATCH`]개씩 묶어 동시에 보낸다. 지연이 큰 회선에서 순서대로 물으면
//!   개수 × 왕복시간이 그대로 대기 시간이 된다.
//!
//! 확인하지 못한 링크는 **예전 그대로**(링크로 표시) 남는다 — 모르는 것을 폴더로 단정하지 않는다.

/// 한 목록에서 대상을 확인할 링크의 최대 개수.
pub(crate) const MAX_RESOLVE: usize = 100;
/// 한 번에 함께 보낼 확인 요청 수.
pub(crate) const BATCH: usize = 8;

/// 확인할 항목들의 자리번호를 고른다(링크만, 상한까지).
///
/// `..`와 `.`은 목록에 링크로 오지 않지만, 와도 따라가지 않는다 — 위로 올라가는 길은
/// 이미 따로 있고, 여기서 확인해 봐야 얻는 것이 없다.
pub(crate) fn to_resolve(kinds: &[nabi_fs::FileKind], names: &[String]) -> Vec<usize> {
    kinds
        .iter()
        .zip(names)
        .enumerate()
        .filter(|(_, (k, n))| matches!(k, nabi_fs::FileKind::Symlink) && n.as_str() != "." && n.as_str() != "..")
        .map(|(i, _)| i)
        .take(MAX_RESOLVE)
        .collect()
}

/// 상한에 걸려 확인하지 못한 링크 수(0이면 전부 확인했다).
pub(crate) fn unresolved(kinds: &[nabi_fs::FileKind]) -> usize {
    let links = kinds.iter().filter(|k| matches!(k, nabi_fs::FileKind::Symlink)).count();
    links.saturating_sub(MAX_RESOLVE)
}

/// 대상 경로를 만든다. 목록 경로가 `/`로 끝나든 아니든 슬래시가 겹치지 않게 한다.
pub(crate) fn target_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_fs::FileKind::{Dir, File, Symlink};

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("n{i}")).collect()
    }

    #[test]
    fn only_links_are_looked_up() {
        let k = [File, Symlink, Dir, Symlink];
        assert_eq!(to_resolve(&k, &names(4)), vec![1, 3]);
    }

    /// **상한이 있다.** 링크가 수백 개인 폴더에서 목록이 몇 초씩 멈추면 안 된다.
    #[test]
    fn the_number_of_lookups_is_capped() {
        let k = vec![Symlink; MAX_RESOLVE + 40];
        assert_eq!(to_resolve(&k, &names(k.len())).len(), MAX_RESOLVE);
        assert_eq!(unresolved(&k), 40, "못 본 개수를 알아야 말해 줄 수 있다");
    }

    #[test]
    fn nothing_is_left_over_below_the_cap() {
        let k = vec![Symlink; 3];
        assert_eq!(unresolved(&k), 0);
    }

    /// 위로 올라가는 항목은 따라가지 않는다 — 얻는 것 없이 왕복만 는다.
    #[test]
    fn the_parent_entry_is_never_followed() {
        let k = [Symlink, Symlink];
        let n = vec!["..".to_string(), "real".to_string()];
        assert_eq!(to_resolve(&k, &n), vec![1]);
    }

    #[test]
    fn paths_join_without_doubling_the_slash() {
        assert_eq!(target_path("/a/b", "c"), "/a/b/c");
        assert_eq!(target_path("/", "c"), "/c");
        assert_eq!(target_path("/a/", "c"), "/a/c");
    }

    #[test]
    fn a_listing_without_links_asks_nothing() {
        let k = [File, Dir, File];
        assert!(to_resolve(&k, &names(3)).is_empty());
    }
}
