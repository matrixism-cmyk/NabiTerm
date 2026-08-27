//! 로컬·원격이 **함께 쓰는 정렬 규칙** — 한 곳에만 둔다(배치 AA).
//!
//! 예전에는 로컬 브라우저(`browserfs::sort_rows`)와 원격 SFTP(`sftpentries::sort_sftp`)가
//! 각자 같은 규칙을 적어 두고 있었다. 그때도 답은 같았지만, 그것이 유지된다는 보장이 없다 —
//! 한쪽만 고쳐지는 순간 **같은 폴더가 로컬 창과 원격 창에서 다른 순서로** 보이고,
//! 그때 사용자는 어느 쪽을 믿어야 할지 모른다.
//!
//! 두 창이 같은 답을 낸다는 것은 `browserfs`의 대조 시험이 지킨다.

use crate::browserfs::{natural_cmp, Sort};

/// 정렬에 필요한 것만 뽑은 항목 — **로컬 파일과 원격 파일이 같은 규칙을 지나게** 한다.
///
/// 예전에는 로컬(`sort_rows`)과 원격(`sort_sftp`)이 각자 같은 규칙을 적어 두고 있었다.
/// 지금은 답이 같지만 그것이 유지된다는 보장이 없다 — 한쪽만 고쳐지는 순간 같은 폴더가
/// 로컬 창과 원격 창에서 다른 순서로 보인다. 그때 사용자는 어느 쪽을 믿어야 할지 모른다.
#[derive(Clone, Copy)]
pub(crate) struct SortKey<'a> {
    pub name: &'a str,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
}

/// 두 항목의 순서 — **유일한 규칙**. 폴더가 먼저, 그다음 기준, `desc`면 기준만 뒤집는다.
///
/// `desc`가 폴더 우선까지 뒤집지 않는 것이 핵심이다. 내림차순이라고 폴더가 아래로 가면
/// 목록을 훑는 방식 자체가 달라진다.
pub(crate) fn entry_cmp(a: SortKey, b: SortKey, sort: Sort, desc: bool) -> std::cmp::Ordering {
    b.is_dir.cmp(&a.is_dir).then_with(|| {
        let ord = match sort {
            Sort::Name => natural_cmp(a.name, b.name),
            Sort::Type => ext_of(a.name).cmp(&ext_of(b.name)).then_with(|| natural_cmp(a.name, b.name)),
            Sort::Size => a.size.cmp(&b.size),
            Sort::Date => a.mtime.cmp(&b.mtime),
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    })
}

/// 확장자(소문자, 없으면 빈 문자열).
pub(crate) fn ext_of(name: &str) -> String {
    std::path::Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 로컬 행에서 정렬 열쇠를 뽑는다.
pub(crate) fn key_of_row(r: &crate::browserfs::Row) -> SortKey<'_> {
    SortKey { name: &r.name, is_dir: r.is_dir, size: r.size, mtime: r.mtime }
}

#[cfg(test)]
mod tests {
    /// **로컬과 원격이 같은 순서를 내는가** — 규칙을 한 곳에 모은 이유가 이것이다.
    ///
    /// 두 창이 같은 폴더를 다르게 늘어놓으면 사용자는 어느 쪽을 믿어야 할지 모른다.
    /// 규칙을 합쳤으니 당연해야 하는데, 그 당연함이 깨지는 순간이 한쪽만 고쳐진 순간이다.
    #[test]
    fn local_and_remote_sort_the_same_way() {
        use crate::browserfs::{sort_rows, Row, Sort};
        use crate::sftpentries::sort_sftp;
        use nabi_proto::SftpEntry;
        // 순서가 갈릴 만한 것들을 일부러 섞는다 — 폴더·확장자·크기·시각·숫자 이름·대소문자.
        let raw: &[(&str, bool, u64, u64)] = &[
            ("b.txt", false, 300, 100),
            ("A.rs", false, 100, 300),
            ("zzz", true, 0, 200),
            ("a10.log", false, 200, 50),
            ("a9.log", false, 200, 50),
            ("Aa", true, 0, 400),
            ("noext", false, 50, 500),
        ];
        let rows: Vec<Row> = raw
            .iter()
            .map(|(n, d, sz, mt)| Row {
                name: (*n).into(), is_dir: *d, is_link: false, size: *sz, mtime: *mt,
            })
            .collect();
        let ents: Vec<SftpEntry> = raw
            .iter()
            .map(|(n, d, sz, mt)| SftpEntry {
                name: (*n).into(), is_dir: *d, is_link: false, size: *sz, mode: 0, mtime: *mt,
            })
            .collect();

        for sort in [Sort::Name, Sort::Type, Sort::Size, Sort::Date] {
            for desc in [false, true] {
                let mut r = rows.clone();
                let mut e = ents.clone();
                sort_rows(&mut r, sort, desc);
                sort_sftp(&mut e, sort, desc);
                let rn: Vec<&str> = r.iter().map(|x| x.name.as_str()).collect();
                let en: Vec<&str> = e.iter().map(|x| x.name.as_str()).collect();
                assert_eq!(rn, en, "정렬 {sort:?} desc={desc} 에서 로컬과 원격이 갈렸다");
            }
        }
    }

    #[test]
    fn folders_stay_first_even_descending() {
        use crate::browserfs::{sort_rows, Row, Sort};
        // 내림차순이라고 폴더가 아래로 가면 목록을 훑는 방식 자체가 달라진다.
        let mut rows = vec![
            Row { name: "file".into(), is_dir: false, is_link: false, size: 1, mtime: 1 },
            Row { name: "dir".into(), is_dir: true, is_link: false, size: 0, mtime: 2 },
        ];
        sort_rows(&mut rows, Sort::Name, true);
        assert!(rows[0].is_dir, "내림차순에서도 폴더가 먼저다");
    }
}
