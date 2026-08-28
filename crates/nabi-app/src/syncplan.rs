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

/// **경로를 빠져나가지 않는가** — `..`·절대경로·드라이브 접두사를 거부한다.
///
/// 원격 서버가 준 이름을 로컬 경로에 join 하기 전 반드시 통과시킬 것(경로 탈출 차단).
///
/// ## 이 판단은 "쓸 수 있는가"와 다르다(배치 AE)
///
/// 예전에는 여기서 **콜론이 있으면 어디에 있든** 거절했다. 드라이브 문자를 막으려던 것인데,
/// 리눅스에서 콜론은 **적법한 파일명 문자**다. 그래서 `2026-08-28T10:00:00.log` 같은 흔한
/// 로그 파일이 원격 찾기·내용 찾기·동기화 목록에서 **통째로 보이지 않았다.** 없다고 나오는
/// 것과 못 쓴다고 나오는 것은 사용자에게 전혀 다른 말이다.
///
/// 이제 콜론은 **드라이브 접두사 자리**에서만 거절한다. 윈도우가 그 이름으로 파일을 못 만드는
/// 문제는 [`writable_on_windows`] 가 따로 판단한다 — 보는 일(찾기)과 쓰는 일(내려받기)은
/// 다른 질문이기 때문이다.
pub fn safe_rel(rel: &str) -> bool {
    let mut parts = rel.split(['/', '\\']);
    // 드라이브 접두사는 **첫 조각에서만** 위험하다. 시험(`what_actually_escapes_when_you_join_it`)
    // 으로 러스트에 직접 물어 확인했다: `join("C:evil")` 은 뿌리를 통째로 갈아치우지만
    // `join("logs/a:b.log")` 는 뿌리 안에 머문다. 접두사는 맨 앞에서만 인정되기 때문이다.
    let first_ok = parts.next().is_some_and(|c| !c.is_empty() && c != ".." && !is_drive(c));
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.starts_with('\\')
        && first_ok
        && parts.all(|c| !c.is_empty() && c != "..")
}

/// 이 조각이 드라이브 접두사인가(`C:` · `c:evil`).
fn is_drive(comp: &str) -> bool {
    let b = comp.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// **윈도우가 이 이름으로 파일을 만들 수 있는가** — 내려받기 전에만 묻는다.
///
/// 리눅스 서버에는 윈도우가 못 쓰는 이름이 흔하다: 콜론(NTFS 는 대체 데이터 스트림 문법으로
/// 읽는다)·물음표·별표·따옴표·부등호·파이프, 그리고 공백이나 점으로 끝나는 이름. 찾기에서는
/// 보여 줘야 하지만 내려받을 때는 **왜 못 받는지 말해 줘야** 한다. 조용히 건너뛰면 사용자는
/// 그 파일이 서버에 없다고 믿는다.
pub fn writable_on_windows(rel: &str) -> bool {
    !rel.split(['/', '\\']).any(|c| {
        c.contains([':', '*', '?', '\"', '<', '>', '|']) || c.ends_with(' ') || c.ends_with('.')
    })
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
    #[test]
    fn a_linux_log_name_with_colons_is_visible() {
        // 이 시험이 이 변경의 이유다. 리눅스에서 콜론은 적법한 파일명 문자라
        // `2026-08-28T10:00:00.log` 같은 흔한 로그가 원격 찾기·내용 찾기·동기화 목록에서
        // 통째로 보이지 않았다. 없다고 나오는 것과 못 쓴다고 나오는 것은 다른 말이다.
        assert!(safe_rel("logs/2026-08-28T10:00:00.log"));
        assert!(safe_rel("backup:2026.tar"));
    }

    #[test]
    fn a_drive_prefix_is_still_rejected() {
        // 콜론을 풀어 주면서 드라이브 탈출까지 열면 안 된다.
        assert!(!safe_rel("C:evil"));
        // 가운데 조각은 탈출하지 않는다(러스트에 물어 확인) — 다만 윈도우에 못 쓸 뿐이다.
        assert!(safe_rel("a/C:evil"));
        assert!(!writable_on_windows("a/C:evil"));
        assert!(!safe_rel("c:/windows/system32"));
    }

    #[test]
    fn windows_cannot_write_some_names_that_linux_allows() {
        // 보는 일과 쓰는 일은 다른 질문이다.
        assert!(!writable_on_windows("logs/2026-08-28T10:00:00.log"), "콜론은 NTFS 가 스트림으로 읽는다");
        assert!(!writable_on_windows("a/what?.txt"));
        assert!(!writable_on_windows("a/trailing "), "공백으로 끝나는 이름");
        assert!(!writable_on_windows("a/trailing."), "점으로 끝나는 이름");
        assert!(writable_on_windows("logs/normal.log"));
        assert!(writable_on_windows("한글/파일.rs"));
    }

    #[test]
    fn the_two_questions_are_independent() {
        // 빠져나가지 않지만 못 쓰는 이름이 있고, 그 반대는 없어야 한다.
        let colon = "logs/a:b.log";
        assert!(safe_rel(colon), "빠져나가지 않는다");
        assert!(!writable_on_windows(colon), "그래도 윈도우에는 못 쓴다");
    }

    #[test]
    fn what_actually_escapes_when_you_join_it() {
        // 짐작하지 말고 러스트에 직접 묻는다 — 어떤 모양이 실제로 뿌리를 벗어나는가.
        use std::path::Path;
        let base = Path::new(r"C:\base");
        // 첫 조각이 드라이브 모양이면 join 이 **뿌리를 통째로 갈아치운다**. 이것이 진짜 위험이다.
        assert!(!base.join("C:evil").starts_with(base), "첫 조각 드라이브는 탈출한다");
        assert!(!base.join("a:b.log").starts_with(base), "한 글자+콜론도 드라이브로 읽힌다");
        // 반면 가운데 조각은 갈아치우지 않는다 — 접두사는 맨 앞에서만 인정된다.
        assert!(base.join("logs/a:b.log").starts_with(base), "가운데 콜론은 벗어나지 않는다");
        assert!(base.join("a/C:evil").starts_with(base), "가운데면 드라이브 모양이어도 안 벗어난다");
    }

}
