//! 팔레트 **최근 사용 우선** — 자주 쓰는 명령이 위로 온다.
//!
//! 명령이 86개다. 목록 순서가 고정이면 늘 쓰는 서너 개를 매번 같은 거리만큼 스크롤하거나
//! 같은 글자를 다시 친다. 팔레트의 값은 "빨리 닿는 것"인데 그 값이 줄어든다.
//!
//! ## 무엇을 기억하는가 — 동작이 아니라 **이름**
//!
//! `PaletteAction`에는 세션 이름·pane 번호·명령문 같은 데이터가 들어 있어 그대로 기억하면
//! 다음 실행 때 같은 것을 가리키지 못한다(pane은 사라지고 번호는 바뀐다). 그래서 화면에
//! 보이던 **이름**을 기억한다. 이름이 같으면 사용자에게는 같은 명령이다.
//!
//! ## 질의 중에도 최근 것이 먼저다
//!
//! 걸러진 뒤에도 순서를 지킨다. Enter가 첫 줄을 고르므로, 방금 쓴 것이 첫 줄이어야
//! 손가락이 기억하는 대로 동작한다.

/// 기억할 개수. 너무 길면 옛날에 한 번 쓴 것이 계속 위에 남아 방해가 된다.
pub(crate) const CAP: usize = 12;

/// 방금 쓴 명령을 맨 앞으로. 이미 있으면 끌어올린다.
pub(crate) fn bump(recent: &mut Vec<String>, label: &str) {
    if label.is_empty() {
        return;
    }
    recent.retain(|r| r != label);
    recent.insert(0, label.to_string());
    recent.truncate(CAP);
}

/// 최근 순 → 나머지 원래 순서. **안정적**이다(같은 무리 안에서는 원래 차례를 지킨다).
pub(crate) fn order<A>(cmds: Vec<(String, A)>, recent: &[String]) -> Vec<(String, A)> {
    let rank = |label: &str| recent.iter().position(|r| r == label);
    let mut hits: Vec<(usize, (String, A))> = Vec::new();
    let mut rest: Vec<(String, A)> = Vec::new();
    for c in cmds {
        match rank(&c.0) {
            Some(i) => hits.push((i, c)),
            None => rest.push(c),
        }
    }
    hits.sort_by_key(|(i, _)| *i);
    let mut out: Vec<(String, A)> = hits.into_iter().map(|(_, c)| c).collect();
    out.extend(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(v: &[(String, u8)]) -> Vec<&str> {
        v.iter().map(|(l, _)| l.as_str()).collect()
    }

    fn cmds(names: &[&str]) -> Vec<(String, u8)> {
        names.iter().enumerate().map(|(i, n)| (n.to_string(), i as u8)).collect()
    }

    #[test]
    fn a_used_command_moves_to_the_front() {
        let mut r = vec!["b".to_string()];
        bump(&mut r, "a");
        assert_eq!(r, vec!["a", "b"]);
    }

    /// 같은 것을 또 쓰면 두 번 쌓이지 않고 끌어올려진다.
    #[test]
    fn using_it_again_lifts_it_instead_of_duplicating() {
        let mut r = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        bump(&mut r, "c");
        assert_eq!(r, vec!["c", "a", "b"]);
    }

    /// 목록이 무한히 자라면 옛날에 한 번 쓴 것이 계속 위를 차지한다.
    #[test]
    fn the_list_stays_bounded() {
        let mut r: Vec<String> = Vec::new();
        for i in 0..40 {
            bump(&mut r, &format!("c{i}"));
        }
        assert_eq!(r.len(), CAP);
        assert_eq!(r[0], "c39", "가장 최근 것이 맨 앞이어야 한다");
    }

    #[test]
    fn an_empty_label_is_ignored() {
        let mut r: Vec<String> = Vec::new();
        bump(&mut r, "");
        assert!(r.is_empty());
    }

    /// 최근 것이 최근 순서대로 앞에 오고, 나머지는 원래 차례를 그대로 지킨다.
    #[test]
    fn recent_first_then_the_original_order() {
        let recent = vec!["d".to_string(), "b".to_string()];
        let got = order(cmds(&["a", "b", "c", "d", "e"]), &recent);
        assert_eq!(labels(&got), vec!["d", "b", "a", "c", "e"]);
    }

    /// **동작이 이름을 따라가야 한다** — 순서만 바꾸고 짝이 어긋나면 엉뚱한 명령이 돈다.
    #[test]
    fn each_label_keeps_its_own_action() {
        let recent = vec!["c".to_string()];
        let got = order(cmds(&["a", "b", "c"]), &recent);
        assert_eq!(got[0], ("c".to_string(), 2u8), "이름만 옮기고 동작은 안 따라왔다");
    }

    #[test]
    fn no_history_means_no_change() {
        let got = order(cmds(&["a", "b", "c"]), &[]);
        assert_eq!(labels(&got), vec!["a", "b", "c"]);
    }

    /// 기억에는 있지만 지금 목록에 없는 이름(사라진 pane 등)은 조용히 무시된다.
    #[test]
    fn a_remembered_name_that_no_longer_exists_is_harmless() {
        let recent = vec!["gone".to_string(), "b".to_string()];
        let got = order(cmds(&["a", "b"]), &recent);
        assert_eq!(labels(&got), vec!["b", "a"]);
    }
}
