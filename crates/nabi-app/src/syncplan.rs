//! 폴더 동기화 계획(S6-51~53, WinSCP식) — 로컬/원격 파일 트리를 비교해 할 일 목록을 만든다.
//!
//! 순수 함수: I/O 없음. 안전 원칙 — 삭제는 Mirror 모드에서만 나오고, UI는 삭제 항목을
//! 기본 체크 해제로 보여 준다(실수로 지우는 것보다 안 지우는 쪽이 항상 싸다).

use std::collections::BTreeMap;

/// 동기화 방향.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncDir {
    /// 로컬 → 원격(업로드).
    Up,
    /// 원격 → 로컬(다운로드).
    Down,
}

/// 비교 기준 — 시각은 ±2초 허용(FAT/SFTP 반올림 관용, WinSCP 기본과 동일 사상).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncBy {
    Size,
    SizeAndTime,
}

/// 한 파일에 대한 판정.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SyncAction {
    /// 원본에만 있음 → 복사(방향대로).
    Copy(String),
    /// 양쪽에 있고 다름 → 갱신(방향대로 덮어씀).
    Update(String),
    /// 대상에만 있음 → Mirror 모드에서만 삭제 후보.
    Delete(String),
}

impl SyncAction {
    pub fn path(&self) -> &str {
        match self {
            SyncAction::Copy(p) | SyncAction::Update(p) | SyncAction::Delete(p) => p,
        }
    }
}

/// (상대경로 → (크기, mtime)) 맵으로 변환(비교 기준 자료).
pub fn to_map(list: &[(String, u64, u64)]) -> BTreeMap<String, (u64, u64)> {
    list.iter().map(|(p, s, m)| (p.clone(), (*s, *m))).collect()
}

/// 원본(src)→대상(dst) 계획: 새 파일=Copy, 다른 파일=Update, 대상에만 있는 파일=Delete(mirror 시).
/// 시각 비교는 원본이 더 최신일 때만 갱신(대상이 더 새로우면 건드리지 않는다 — 안전).
pub fn plan(
    src: &BTreeMap<String, (u64, u64)>,
    dst: &BTreeMap<String, (u64, u64)>,
    by: SyncBy,
    mirror: bool,
) -> Vec<SyncAction> {
    let mut out = Vec::new();
    for (p, (ss, sm)) in src {
        match dst.get(p) {
            None => out.push(SyncAction::Copy(p.clone())),
            Some((ds, dm)) => {
                // 크기가 다르면 무조건 갱신(시각 보존 도구·mtime 되돌림도 놓치지 않게).
                // 크기가 같고 시각만 다르면 원본이 더 최신일 때만(±2초 관용, 대상 최신 보호).
                let update = ss != ds
                    || (by == SyncBy::SizeAndTime && sm.saturating_sub(*dm) > 2);
                if update {
                    out.push(SyncAction::Update(p.clone()));
                }
            }
        }
    }
    if mirror {
        for p in dst.keys() {
            if !src.contains_key(p) {
                out.push(SyncAction::Delete(p.clone()));
            }
        }
    }
    out
}

/// 동기화에 안전한 상대경로인가 — `..`/절대경로/드라이브 문자를 거부한다.
/// 원격 서버가 준 이름을 로컬 경로에 join하기 전 반드시 통과시킬 것(경로 탈출 차단).
pub fn safe_rel(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.starts_with('\\')
        && !rel.contains(':')
        && rel.split(['/', '\\']).all(|c| !c.is_empty() && c != "..")
}

/// 로컬 디렉터리 트리를 (상대경로, 크기, mtime)로 수집(원격 list_tree와 짝).
pub fn walk_local(root: &std::path::Path) -> Vec<(String, u64, u64)> {
    walk_local_capped(root, usize::MAX).unwrap_or_default()
}

/// 상한부 수집 — cap 초과를 발견한 즉시 중단하고 None(대형 트리 UI 프리즈 방지).
pub fn walk_local_capped(root: &std::path::Path, cap: usize) -> Option<Vec<(String, u64, u64)>> {
    let mut out = Vec::new();
    if !walk_capped(root, "", &mut out, cap) {
        return None;
    }
    out.sort();
    Some(out)
}

fn walk_capped(dir: &std::path::Path, prefix: &str, out: &mut Vec<(String, u64, u64)>, cap: usize) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return true };
    for e in rd.flatten() {
        if out.len() >= cap {
            return false; // 상한 도달 — 즉시 중단(전체 순회 금지).
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            if !walk_capped(&e.path(), &rel, out, cap) {
                return false;
            }
        } else {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push((rel, meta.len(), mtime));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: &[(&str, u64, u64)]) -> BTreeMap<String, (u64, u64)> {
        v.iter().map(|(p, s, t)| (p.to_string(), (*s, *t))).collect()
    }

    #[test]
    fn plans_copy_update_delete() {
        let src = m(&[("a.txt", 10, 100), ("b/새파일.txt", 5, 200), ("same.txt", 7, 50)]);
        let dst = m(&[("a.txt", 10, 90), ("only_dst.txt", 3, 10), ("same.txt", 7, 50)]);
        let acts = plan(&src, &dst, SyncBy::SizeAndTime, true);
        assert!(acts.contains(&SyncAction::Update("a.txt".into())), "원본이 더 최신+시각차>2 → 갱신");
        assert!(acts.contains(&SyncAction::Copy("b/새파일.txt".into())));
        assert!(acts.contains(&SyncAction::Delete("only_dst.txt".into())), "mirror만 삭제");
        assert!(!acts.iter().any(|a| a.path() == "same.txt"), "동일 파일 무동작");
        // mirror 끄면 삭제 없음.
        assert!(!plan(&src, &dst, SyncBy::SizeAndTime, false).iter().any(|a| matches!(a, SyncAction::Delete(_))));
    }

    #[test]
    fn time_tolerance_and_direction_safety() {
        // ±2초 이내 차이는 같은 것으로(FAT/SFTP 반올림).
        let src = m(&[("x", 4, 102)]);
        let dst = m(&[("x", 4, 100)]);
        assert!(plan(&src, &dst, SyncBy::SizeAndTime, false).is_empty(), "2초 차=동일");
        // 크기가 다르면 대상이 최신이어도 갱신한다(변경 누락이 더 위험 — 리뷰 #8).
        let src2 = m(&[("y", 9, 100)]);
        let dst2 = m(&[("y", 4, 999)]);
        assert_eq!(plan(&src2, &dst2, SyncBy::SizeAndTime, false).len(), 1, "크기 불일치=갱신");
        // 크기가 같고 대상이 더 최신이면 보호(덮어쓰지 않음).
        let src3 = m(&[("z", 7, 100)]);
        let dst3 = m(&[("z", 7, 999)]);
        assert!(plan(&src3, &dst3, SyncBy::SizeAndTime, false).is_empty(), "동일 크기+대상 최신 보호");
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        assert!(safe_rel("a/b.txt") && safe_rel("한글/파일.rs"));
        for bad in ["../x", "a/../b", "/etc/passwd", "..", "C:evil", "a\\..\\b", ""] {
            assert!(!safe_rel(bad), "{bad}");
        }
    }

    #[test]
    fn local_walk_collects_relative_paths() {
        let d = std::env::temp_dir().join(format!("nabi-syncwalk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("a.txt"), b"12345").unwrap();
        std::fs::write(d.join("sub").join("한글.txt"), b"xy").unwrap();
        let got = walk_local(&d);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "a.txt");
        assert_eq!(got[1].0, "sub/한글.txt");
        assert_eq!(got[1].1, 2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
