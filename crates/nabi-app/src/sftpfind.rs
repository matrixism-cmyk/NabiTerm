//! **원격 파일 찾기** — 지금 폴더 아래에서 이름으로 찾는다.
//!
//! 서버에서 파일을 찾는 길은 지금까지 두 가지뿐이었다. 목록을 손으로 파고들거나, 터미널
//! 창으로 건너가 `find`를 치거나. 파일 관리자를 쓰던 흐름이 거기서 끊긴다.
//!
//! ## 새로 만들지 않았다
//!
//! 원격 트리를 훑는 일은 이미 있다 — 동기화가 쓰는 `SftpListTree`가 상대경로·크기·시각을
//! 통째로 돌려준다. 찾기는 **그 결과를 거르는 것**이므로 새 명령도 새 재귀도 필요 없다.
//! (같은 일을 두 경로가 각각 하면 언젠가 서로 다른 답을 낸다.)
//!
//! ## 무엇으로 맞추나
//!
//! 파일 이름만 본다 — 경로 전체로 맞추면 상위 폴더 이름 때문에 안에 있는 것이 전부 걸린다.
//! `*`와 `?`를 쓰는 사람이 많아 글로브를 함께 받고, 특수문자가 없으면 그냥 부분 일치로 본다.
//! 대소문자는 가리지 않는다(서버 파일 이름 표기는 제각각이다).

/// 찾은 것 하나 — 트리가 준 상대경로 그대로.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub rel: String,
    pub size: u64,
    pub mtime: u64,
}

/// 화면에 담을 최대 개수. 넘으면 **넘었다고 말한다**(조용히 자르지 않는다).
pub(crate) const MAX: usize = 2000;

/// 이름이 질의에 맞는가. `*`·`?`가 있으면 글로브, 없으면 부분 일치. 대소문자 무시.
pub(crate) fn matches(name: &str, query: &str) -> bool {
    let (n, q) = (name.to_lowercase(), query.to_lowercase());
    if q.is_empty() {
        return false;
    }
    if q.contains('*') || q.contains('?') {
        glob(&n, &q)
    } else {
        n.contains(&q)
    }
}

/// 아주 작은 글로브 — `*`(0자 이상)와 `?`(정확히 1자)만. 되돌아가며 맞춘다.
fn glob(name: &str, pat: &str) -> bool {
    let (n, p): (Vec<char>, Vec<char>) = (name.chars().collect(), pat.chars().collect());
    // (이름 위치, 패턴 위치)를 되짚기 위한 표식 — `*`를 만나면 여기로 돌아온다.
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while i < n.len() {
        if j < p.len() && (p[j] == '?' || p[j] == n[i]) {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '*' {
            star = j;
            mark = i;
            j += 1;
        } else if star != usize::MAX {
            j = star + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '*' {
        j += 1;
    }
    j == p.len()
}

/// 트리 결과를 걸러 낸다. `(찾은 것, 상한에 걸려 못 담은 수)`.
pub(crate) fn filter(files: &[(String, u64, u64)], query: &str) -> (Vec<Hit>, usize) {
    let mut out = Vec::new();
    let mut cut = 0usize;
    for (rel, size, mtime) in files {
        // 이름만 본다 — 상위 폴더 이름으로 걸리면 안 된다.
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if !matches(name, query) {
            continue;
        }
        if out.len() < MAX {
            out.push(Hit { rel: rel.clone(), size: *size, mtime: *mtime });
        } else {
            cut += 1;
        }
    }
    (out, cut)
}

/// 찾은 항목이 있는 폴더 경로(그리로 이동해 보여 주기 위해). 최상위면 `""`.
pub(crate) fn parent_of(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<(String, u64, u64)> {
        [
            ("app/config.toml", 10u64),
            ("app/logs/error.log", 20),
            ("app/logs/access.log", 30),
            ("data/backup.tar.gz", 40),
            ("README.md", 50),
        ]
        .iter()
        .map(|(p, s)| (p.to_string(), *s, 0u64))
        .collect()
    }

    #[test]
    fn a_plain_word_matches_anywhere_in_the_name() {
        let (h, _) = filter(&tree(), "log");
        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|x| x.rel.ends_with(".log")));
    }

    /// **폴더 이름으로 걸리면 안 된다.** `app/`ㅇ 아래 것이 전부 나오면 찾기가 아니다.
    #[test]
    fn a_parent_folder_name_does_not_drag_its_children_in() {
        let (h, _) = filter(&tree(), "app");
        assert!(h.is_empty(), "폴더 이름에 걸려 자식이 나왔다: {h:?}");
    }

    #[test]
    fn case_is_ignored() {
        let (h, _) = filter(&tree(), "README");
        assert_eq!(h.len(), 1);
        let (h2, _) = filter(&tree(), "readme");
        assert_eq!(h2.len(), 1);
    }

    #[test]
    fn a_star_glob_works() {
        let (h, _) = filter(&tree(), "*.log");
        assert_eq!(h.len(), 2);
        let (h2, _) = filter(&tree(), "conf*");
        assert_eq!(h2.len(), 1);
    }

    #[test]
    fn a_question_mark_matches_exactly_one_character() {
        assert!(matches("a.log", "?.log"));
        assert!(!matches("ab.log", "?.log"));
    }

    /// 글로브는 이름 **전체**에 맞아야 한다 — 부분 일치와 섞이면 결과를 못 믿는다.
    #[test]
    fn a_glob_anchors_to_the_whole_name() {
        assert!(!matches("error.log.1", "*.log"));
        assert!(matches("error.log.1", "*.log*"));
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        assert!(!matches("anything", ""));
        assert!(filter(&tree(), "").0.is_empty());
    }

    /// 상한에 걸리면 **몇 개를 못 보여 줬는지** 말한다.
    #[test]
    fn hitting_the_cap_is_reported() {
        let big: Vec<(String, u64, u64)> =
            (0..MAX + 17).map(|i| (format!("d/f{i}.log"), 1, 0)).collect();
        let (h, cut) = filter(&big, "*.log");
        assert_eq!(h.len(), MAX);
        assert_eq!(cut, 17);
    }

    #[test]
    fn the_containing_folder_is_derived_from_the_path() {
        assert_eq!(parent_of("app/logs/error.log"), "app/logs");
        assert_eq!(parent_of("README.md"), "");
    }
}
