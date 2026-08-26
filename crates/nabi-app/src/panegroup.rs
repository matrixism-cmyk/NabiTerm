//! **"그룹"이 무엇을 뜻하는지 한 곳에서 정한다.**
//!
//! 동시 입력(브로드캐스트)이 오래전부터 쓰던 규칙이 있다 — 그룹을 지정하지 않았으면
//! **이 창의 터미널 전부**가 대상이다. 그 규칙이 함수 안에 묻혀 있었는데, 동기 스크롤이
//! 같은 뜻을 필요로 하면서 두 곳이 되게 생겼다.
//!
//! 두 곳이 되면 언젠가 어긋난다. 그러면 "입력은 넷에 갔는데 스크롤은 셋만 움직이는" 일이
//! 벌어지고, 사용자는 어느 쪽이 맞는지 알 수 없다. 그래서 규칙을 밖으로 꺼내 시험을 붙인다.

use nabi_types::PaneId;
use std::collections::HashSet;

/// 이번 동작이 닿을 pane들. 그룹이 비어 있으면 이 창의 터미널 전부.
///
/// 창에 없는 pane이 그룹에 남아 있을 수 있다(다른 창으로 옮겼거나 닫힌 뒤 늦게 정리됨).
/// 그런 것은 **거른다** — 안 그러면 보이지 않는 곳에 입력이 간다.
pub(crate) fn targets(group: &HashSet<PaneId>, window: &HashSet<PaneId>) -> Vec<PaneId> {
    let mut v: Vec<PaneId> = match group.is_empty() {
        true => window.iter().copied().collect(),
        false => group.iter().copied().filter(|p| window.contains(p)).collect(),
    };
    // 순서를 정해 둔다 — 집합은 순서가 없어 같은 입력에 매번 다른 차례로 보내게 된다.
    v.sort_by_key(|p| p.get());
    v
}

#[cfg(test)]
mod tests {
    use super::targets;
    use nabi_types::PaneId;
    use std::collections::HashSet;

    fn set(ns: &[u64]) -> HashSet<PaneId> {
        ns.iter().map(|n| PaneId::new(*n)).collect()
    }

    /// 그룹을 안 정했으면 **이 창 전부** — 테두리로 표시하는 대상과 정확히 같아야 한다.
    #[test]
    fn an_empty_group_means_every_terminal_in_this_window() {
        let got = targets(&HashSet::new(), &set(&[3, 1, 2]));
        assert_eq!(got, vec![PaneId::new(1), PaneId::new(2), PaneId::new(3)]);
    }

    #[test]
    fn a_named_group_wins_over_the_window() {
        let got = targets(&set(&[2]), &set(&[1, 2, 3]));
        assert_eq!(got, vec![PaneId::new(2)]);
    }

    /// **이 창에 없는 pane은 거른다** — 보이지 않는 곳에 입력이 가면 안 된다.
    #[test]
    fn a_pane_that_left_this_window_is_dropped() {
        let got = targets(&set(&[2, 99]), &set(&[1, 2, 3]));
        assert_eq!(got, vec![PaneId::new(2)], "다른 창의 pane이 대상에 남았다");
    }

    /// 차례가 매번 같아야 한다(집합은 순서가 없다).
    #[test]
    fn the_order_is_stable() {
        let (g, w) = (set(&[5, 1, 3]), set(&[1, 3, 5, 7]));
        assert_eq!(targets(&g, &w), targets(&g, &w));
        assert_eq!(targets(&g, &w).first(), Some(&PaneId::new(1)));
    }

    /// 창이 비었으면 아무 데도 안 보낸다(빈 목록이지 오류가 아니다).
    #[test]
    fn nothing_to_send_to_is_not_an_error() {
        assert!(targets(&HashSet::new(), &HashSet::new()).is_empty());
        assert!(targets(&set(&[1]), &HashSet::new()).is_empty());
    }
}
