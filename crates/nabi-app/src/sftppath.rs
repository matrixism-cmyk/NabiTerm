//! 원격(POSIX) 경로 헬퍼 — SFTP 패널용. 원격은 항상 `/` 구분자.

/// `base+ext`가 이미 있으면 `base (1)+ext`, `base (2)+ext`… 처럼 빈 이름을 찾는다(새 폴더/파일 기본명 중복 회피).
/// `taken`은 그 이름이 이미 존재하는지 판정(파일시스템 또는 목록). 로컬·SFTP 공용.
pub(crate) fn dedup_name(taken: impl Fn(&str) -> bool, base: &str, ext: &str) -> String {
    let first = format!("{base}{ext}");
    if !taken(&first) { return first; }
    (1..).map(|i| format!("{base} ({i}){ext}")).find(|n| !taken(n)).unwrap_or(first)
}

/// 디렉터리 경로 결합.
pub(crate) fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

/// 원격 경로의 마지막 구성요소(다운로드 폴더명용). 비거나 "."이면 "sftp".
pub(crate) fn remote_basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && *s != ".")
        .unwrap_or("sftp")
        .to_string()
}

/// 파일/폴더 이름으로 유효한지(이름변경·새 폴더/파일 입력 검증).
/// 빈 문자열·"."·".."·구분자('/','\\') 포함은 거부한다.
pub(crate) fn valid_name(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t != "." && t != ".." && !t.contains('/') && !t.contains('\\')
}

/// 경로를 브레드크럼 세그먼트로 분해: 각 (표시 라벨, 그 지점까지의 절대경로).
/// "/home/u" → [("home","/home"), ("u","/home/u")]. 루트(/)는 호출측이 따로 그린다.
pub(crate) fn crumbs(path: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut acc = String::new();
    for seg in path.split('/').filter(|s| !s.is_empty() && *s != ".") {
        acc.push('/');
        acc.push_str(seg);
        out.push((seg.to_string(), acc.clone()));
    }
    out
}

/// 경로의 상위 디렉터리(POSIX). 루트면 "/".
pub(crate) fn parent_dir(p: &str) -> String {
    match p.trim_end_matches('/').rsplit_once('/') {
        Some(("", _)) | None => "/".to_string(),
        Some((parent, _)) => parent.to_string(),
    }
}

/// 원격 경로 정규화: 중복 `/` 제거, `.` 제거, `..`는 상위로 접는다(절대/상대 보존).
pub(crate) fn normalize(path: &str) -> String {
    let abs = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if matches!(out.last(), Some(&s) if s != "..") {
                    out.pop();
                } else if !abs {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if abs {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::{crumbs, dedup_name, join_path, normalize, parent_dir, remote_basename, valid_name};

    #[test]
    fn dedup_name_appends_sequence() {
        let taken: std::collections::HashSet<&str> = ["새 폴더", "새 폴더 (1)", "새 파일.txt"].into_iter().collect();
        let t = |n: &str| taken.contains(n);
        assert_eq!(dedup_name(t, "새 폴더", ""), "새 폴더 (2)"); // (1)까지 차서 (2).
        assert_eq!(dedup_name(t, "새 파일", ".txt"), "새 파일 (1).txt"); // 확장자 앞 순번.
        assert_eq!(dedup_name(t, "빈자리", ""), "빈자리"); // 없으면 그대로.
    }

    #[test]
    fn normalize_resolves() {
        assert_eq!(normalize("/a/b/../c"), "/a/c");
        assert_eq!(normalize("/a//b/./c/"), "/a/b/c");
        assert_eq!(normalize("/.."), "/");
        assert_eq!(normalize("a/../.."), "..");
        assert_eq!(normalize("/home/user"), "/home/user");
    }

    #[test]
    fn valid_name_rejects_bad() {
        assert!(valid_name("file.txt"));
        assert!(valid_name(" report ")); // 트림 후 유효.
        assert!(!valid_name(""));
        assert!(!valid_name("  "));
        assert!(!valid_name("."));
        assert!(!valid_name(".."));
        assert!(!valid_name("a/b"));
        assert!(!valid_name("a\\b"));
    }

    #[test]
    fn crumbs_builds_prefixes() {
        assert_eq!(
            crumbs("/home/u/src"),
            vec![
                ("home".into(), "/home".into()),
                ("u".into(), "/home/u".into()),
                ("src".into(), "/home/u/src".into()),
            ]
        );
        assert!(crumbs("/").is_empty());
    }

    #[test]
    fn join_path_posix() {
        assert_eq!(join_path("/home", "u"), "/home/u");
        assert_eq!(join_path("/", "etc"), "/etc");
        assert_eq!(join_path("/a/b", "c.txt"), "/a/b/c.txt");
    }

    #[test]
    fn remote_basename_last_segment() {
        assert_eq!(remote_basename("/home/user/proj"), "proj");
        assert_eq!(remote_basename("/home/user/proj/"), "proj");
        assert_eq!(remote_basename("/"), "sftp");
        assert_eq!(remote_basename("."), "sftp");
    }

    #[test]
    fn parent_dir_climbs() {
        assert_eq!(parent_dir("/home/user/docs"), "/home/user");
        assert_eq!(parent_dir("/home/user/"), "/home"); // 후행 슬래시 무시.
        assert_eq!(parent_dir("/home"), "/"); // 최상위 직전 → 루트.
        assert_eq!(parent_dir("/"), "/"); // 루트의 부모는 루트.
    }
}
