//! **이름만 바뀐 파일을 알아본다** — 다시 올리지 않고 저쪽에서 옮긴다.
//!
//! ## 무엇을 푸는가
//!
//! 폴더 이름 하나를 바꾸고 동기화하면, 계획은 그 안의 모든 파일을 "저쪽에 없는 새 파일"
//! (`Copy`)로 보고 옛 자리의 같은 파일들을 "이쪽에 없는 파일"(`Delete`)로 본다.
//! 내용은 한 글자도 안 바뀌었는데 **전부 다시 올린다.** 5GB 폴더의 이름을 바꾸면 5GB 를
//! 다시 보낸다. WinSCP 가 최근 판에서 손본 것도 바로 이 자리다(2026-09-01 조사).
//!
//! 여기서는 `Copy` 하나와 `Delete` 하나가 **같은 파일**임을 알아보면 둘을 지우고
//! [`SyncAction::Move`] 하나로 바꾼다. 실행하는 쪽은 올리는 대신 **이름을 바꾼다** —
//! 바이트가 선을 타지 않으므로 크기와 무관하게 즉시 끝난다.
//!
//! ## 무엇을 같은 파일로 보는가 — 그리고 왜 이렇게 깐깐한가
//!
//! 열쇠는 **(크기, 수정시각)** 이다. 내용을 견주려면 양쪽을 다 읽어야 하는데, 그러면
//! 다시 올리는 것과 비용이 같아져 목적이 사라진다.
//!
//! 잘못 짝지으면 **엉뚱한 파일이 사라진다**(옛 자리에서 옮겨 가므로). 그래서 셋을 건다.
//!
//! 1. **짝이 유일할 때만.** 같은 열쇠를 가진 `Copy` 가 둘이거나 `Delete` 가 둘이면
//!    어느 것이 어느 것인지 알 수 없다 — 그냥 예전처럼 올리고 지운다.
//! 2. **크기가 0 이면 안 센다.** 빈 파일은 서로 구별되지 않아 아무 짝이나 맞는다.
//! 3. **시각이 정확히 같아야 한다.** 계획(`plan`)의 ±2초 관용은 여기 없다 —
//!    거기서는 틀려도 한 번 더 올릴 뿐이지만, 여기서는 틀리면 지운다.
//!
//! 짝을 못 지어도 손해는 없다. 예전 그대로 올리고 지운다.

use crate::syncplan::SyncAction;
use std::collections::{BTreeMap, BTreeSet};

/// 계획에서 이동을 찾아 `Copy`+`Delete` 짝을 [`SyncAction::Move`] 로 바꾼다.
///
/// `src`·`dst` 는 [`crate::syncplan::plan`] 에 넘긴 그 지도여야 한다(크기·시각을 여기서 읽는다).
/// `Delete` 가 없는 계획(미러 아님)은 그대로 돌아온다 — 옮길 옛 자리가 없기 때문이다.
pub fn detect_moves(
    acts: Vec<SyncAction>,
    src: &BTreeMap<String, (u64, u64)>,
    dst: &BTreeMap<String, (u64, u64)>,
) -> Vec<SyncAction> {
    let (mut adds, mut dels) = (BTreeMap::new(), BTreeMap::new());
    for a in &acts {
        match a {
            SyncAction::Copy(p) => push(&mut adds, key(src.get(p)), p),
            SyncAction::Delete(p) => push(&mut dels, key(dst.get(p)), p),
            _ => {}
        }
    }
    // 새 경로 -> 옛 경로. 양쪽 다 후보가 하나뿐일 때만 넣는다.
    let mut pair: BTreeMap<String, String> = BTreeMap::new();
    let mut gone: BTreeSet<String> = BTreeSet::new();
    for (k, tos) in &adds {
        let Some(froms) = dels.get(k) else { continue };
        if tos.len() != 1 || froms.len() != 1 {
            continue;
        }
        pair.insert(tos[0].clone(), froms[0].clone());
        gone.insert(froms[0].clone());
    }
    if pair.is_empty() {
        return acts;
    }
    acts.into_iter()
        .filter_map(|a| match a {
            SyncAction::Copy(p) => Some(match pair.remove(&p) {
                Some(from) => SyncAction::Move { from, to: p },
                None => SyncAction::Copy(p),
            }),
            // 옛 자리는 이동이 대신 처리한다 — 삭제 항목을 남기면 두 번 지운다.
            SyncAction::Delete(p) if gone.contains(&p) => None,
            other => Some(other),
        })
        .collect()
}

/// 이 파일의 짝 열쇠 — 크기가 0 이거나 지도에 없으면 후보가 아니다.
fn key(e: Option<&(u64, u64)>) -> Option<(u64, u64)> {
    match e {
        Some(&(size, mtime)) if size > 0 => Some((size, mtime)),
        _ => None,
    }
}

fn push(map: &mut BTreeMap<(u64, u64), Vec<String>>, k: Option<(u64, u64)>, p: &str) {
    if let Some(k) = k {
        map.entry(k).or_default().push(p.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncplan::{plan, SyncBy};

    fn m(v: &[(&str, u64, u64)]) -> BTreeMap<String, (u64, u64)> {
        v.iter().map(|(p, s, t)| (p.to_string(), (*s, *t))).collect()
    }

    /// 계획을 손으로 짜지 않는다 — 진짜 `plan` 이 내놓은 것을 받아야 쓰는 쪽과 같은 길이다.
    fn planned(src: &BTreeMap<String, (u64, u64)>, dst: &BTreeMap<String, (u64, u64)>) -> Vec<SyncAction> {
        detect_moves(plan(src, dst, SyncBy::SizeAndTime, true), src, dst)
    }

    /// 폴더 이름만 바꾼 흔한 경우 — 다시 올리지 않는다.
    #[test]
    fn renaming_a_folder_moves_instead_of_reuploading() {
        let src = m(&[("새이름/a.bin", 5000, 100), ("새이름/b.bin", 7000, 200)]);
        let dst = m(&[("옛이름/a.bin", 5000, 100), ("옛이름/b.bin", 7000, 200)]);
        let acts = planned(&src, &dst);
        assert_eq!(acts.len(), 2, "올리기+지우기 넷이 이동 둘로 줄어야 한다: {acts:?}");
        assert!(acts.contains(&SyncAction::Move { from: "옛이름/a.bin".into(), to: "새이름/a.bin".into() }));
        assert!(acts.contains(&SyncAction::Move { from: "옛이름/b.bin".into(), to: "새이름/b.bin".into() }));
        assert!(!acts.iter().any(|a| matches!(a, SyncAction::Copy(_) | SyncAction::Delete(_))));
    }

    /// **짝이 둘이면 손대지 않는다.** 어느 것이 어느 것인지 모르는 채로 지우면 안 된다.
    #[test]
    fn an_ambiguous_pair_is_left_alone() {
        let src = m(&[("새/a", 900, 10), ("새/b", 900, 10)]);
        let dst = m(&[("옛/a", 900, 10), ("옛/b", 900, 10)]);
        let acts = planned(&src, &dst);
        assert!(!acts.iter().any(|a| matches!(a, SyncAction::Move { .. })), "모호한데 옮겼다: {acts:?}");
        assert_eq!(acts.len(), 4, "예전처럼 올리고 지운다");
    }

    /// 빈 파일은 서로 구별되지 않는다 — 아무 짝이나 맞아 버린다.
    #[test]
    fn empty_files_are_never_paired() {
        let src = m(&[("새/빈", 0, 10)]);
        let dst = m(&[("옛/빈", 0, 10)]);
        assert!(!planned(&src, &dst).iter().any(|a| matches!(a, SyncAction::Move { .. })));
    }

    /// 시각이 1초라도 다르면 짝이 아니다 — 계획의 ±2초 관용은 여기 없다.
    #[test]
    fn a_one_second_difference_is_not_a_move() {
        let src = m(&[("새/a", 500, 101)]);
        let dst = m(&[("옛/a", 500, 100)]);
        assert!(!planned(&src, &dst).iter().any(|a| matches!(a, SyncAction::Move { .. })));
    }

    /// 미러가 아니면 지울 옛 자리가 없다 — 이동도 없다.
    #[test]
    fn without_mirror_there_is_nothing_to_move_from() {
        let src = m(&[("새/a", 500, 100)]);
        let dst = m(&[("옛/a", 500, 100)]);
        let acts = detect_moves(plan(&src, &dst, SyncBy::SizeAndTime, false), &src, &dst);
        assert_eq!(acts, vec![SyncAction::Copy("새/a".into())]);
    }

    /// 이동으로 바뀐 옛 자리는 삭제 목록에서 **빠져야** 한다 — 남으면 옮긴 뒤 또 지운다.
    #[test]
    fn the_old_path_does_not_also_get_deleted() {
        let src = m(&[("새/a", 500, 100), ("남는것", 3, 1)]);
        let dst = m(&[("옛/a", 500, 100), ("정말없어진것", 42, 7)]);
        let acts = planned(&src, &dst);
        let dels: Vec<&str> = acts.iter().filter_map(|a| match a {
            SyncAction::Delete(p) => Some(p.as_str()),
            _ => None,
        }).collect();
        assert_eq!(dels, ["정말없어진것"], "옮긴 자리를 또 지운다: {acts:?}");
    }

    /// 갱신·삭제 등 나머지 판정은 건드리지 않는다.
    #[test]
    fn other_actions_pass_through_untouched() {
        let src = m(&[("바뀐것", 10, 500)]);
        let dst = m(&[("바뀐것", 20, 100)]);
        assert_eq!(planned(&src, &dst), vec![SyncAction::Update("바뀐것".into())]);
    }
}
