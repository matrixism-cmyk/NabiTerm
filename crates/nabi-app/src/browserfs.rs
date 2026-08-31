//! 로컬 파일시스템 헬퍼 — 디렉터리 항목 읽기·정렬.

use std::path::Path;

// 포맷 헬퍼는 humanfmt에서 정의(재export로 기존 crate::browserfs::* 경로 유지).
pub(crate) use crate::humanfmt::{human, human_age, human_datetime, local_crumbs, summarize};

/// 디렉터리 한 항목.
#[derive(Clone)]
pub(crate) struct Row {
    pub name: String,
    pub is_dir: bool,
    /// 심볼릭 링크 여부(아이콘 구분). is_dir은 대상 기준(따라가므로 dir 링크는 진입 가능).
    pub is_link: bool,
    pub size: u64,
    /// 수정 시각(unix 초, 0=알 수 없음).
    pub mtime: u64,
}

/// SystemTime → unix 초(실패 시 0).
fn to_unix(t: std::io::Result<std::time::SystemTime>) -> u64 {
    t.ok()
        .and_then(|st| st.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 정렬 기준(폴더는 항상 먼저). 이름=오름차순, 크기·날짜=내림차순(큰/최신 먼저).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sort {
    Name,
    Type,
    Size,
    Date,
}

impl Sort {
    /// 다음 정렬 기준으로 순환(버튼 토글용).
    pub(crate) fn next(self) -> Sort {
        match self {
            Sort::Name => Sort::Type,
            Sort::Type => Sort::Size,
            Sort::Size => Sort::Date,
            Sort::Date => Sort::Name,
        }
    }
    /// 현재 기준의 i18n 키.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Sort::Name => "sort.name",
            Sort::Type => "browser.col.type",
            Sort::Size => "sort.size",
            Sort::Date => "sort.date",
        }
    }
    /// 설정 영속용 정수 표현.
    pub(crate) fn to_u8(self) -> u8 {
        match self {
            Sort::Name => 0,
            Sort::Size => 1,
            Sort::Date => 2,
            Sort::Type => 3,
        }
    }
    pub(crate) fn from_u8(n: u8) -> Sort {
        match n {
            1 => Sort::Size,
            2 => Sort::Date,
            3 => Sort::Type,
            _ => Sort::Name,
        }
    }
}

/// 폴더 우선 + 지정 기준으로 행을 정렬한다(순수 — 테스트 가능).
/// 자연 정렬 비교: 숫자 런은 수치로(file2 < file10), 나머지는 대소문자 무시.
pub(crate) fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ac = a.chars().peekable();
    let mut bc = b.chars().peekable();
    loop {
        match (ac.peek().copied(), bc.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut xn = 0u64;
                while let Some(d) = ac.peek().copied().filter(|c| c.is_ascii_digit()) {
                    xn = xn.saturating_mul(10).saturating_add((d as u8 - b'0') as u64);
                    ac.next();
                }
                let mut yn = 0u64;
                while let Some(d) = bc.peek().copied().filter(|c| c.is_ascii_digit()) {
                    yn = yn.saturating_mul(10).saturating_add((d as u8 - b'0') as u64);
                    bc.next();
                }
                if xn != yn {
                    return xn.cmp(&yn);
                }
            }
            (Some(x), Some(y)) => {
                let (xl, yl) = (x.to_ascii_lowercase(), y.to_ascii_lowercase());
                if xl != yl {
                    return xl.cmp(&yl);
                }
                ac.next();
                bc.next();
            }
        }
    }
}

/// 폴더 우선 + 지정 기준으로 정렬한다(desc면 기준 방향을 뒤집되 폴더 우선은 유지).
pub(crate) fn sort_rows(rows: &mut [Row], sort: Sort, desc: bool) {
    rows.sort_by(|a, b| {
        crate::browsersort::entry_cmp(
            crate::browsersort::key_of_row(a),
            crate::browsersort::key_of_row(b),
            sort,
            desc,
        )
    });
}


/// 숨김 항목인가: `.` 접두(유닉스식) 또는 Windows HIDDEN 속성(desktop.ini 등).
fn is_hidden(name: &str, md: Option<&std::fs::Metadata>) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    if let Some(m) = md {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        return m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
    }
    let _ = md;
    false
}

/// 디렉터리를 읽어 폴더 우선 + 지정 기준으로 정렬한 항목 목록을 돌려준다.
/// show_hidden=false면 `.` 접두 또는 Windows HIDDEN 속성 항목을 제외한다.
pub(crate) fn read_entries(path: &Path, sort: Sort, desc: bool, show_hidden: bool) -> Vec<Row> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let md = e.metadata().ok();
            if !show_hidden && is_hidden(&name, md.as_ref()) {
                continue;
            }
            let is_link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false); // file_type는 링크 미추적.
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let (size, mtime) = (md.as_ref().map(|m| m.len()).unwrap_or(0), md.map(|m| to_unix(m.modified())).unwrap_or(0));
            out.push(Row { name, is_dir, is_link, size, mtime });
        }
    }
    sort_rows(&mut out, sort, desc);
    out
}

#[cfg(test)]
mod tests {
    /// **폴더 하나를 읽는 데 얼마나 드는가** — 캐시를 둘 근거를 숫자로 남긴다.
    ///
    /// 항목마다 `metadata()` 를 부르므로 파일 수만큼 시스템 호출이 난다. 화면은 초당 예순
    /// 번 그려지니, 캐시가 없으면 그 수만큼 곱해진다. 네트워크 경로(UNC·폐쇄망 공유)에서는
    /// 한 번이 곧 왕복이라 더 나쁘다.
    ///
    /// 평소에는 돌리지 않는다(파일 2천 개를 만든다) — 숫자가 궁금할 때만:
    /// `cargo test -p nabi-app how_long -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn how_long_does_a_big_folder_take() {
        use super::{read_entries, Sort};
        let dir = std::env::temp_dir().join(format!("nabi-bench-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for i in 0..2000 {
            let _ = std::fs::write(dir.join(format!("f{i:04}.txt")), b"x");
        }
        let t = std::time::Instant::now();
        let rows = read_entries(&dir, Sort::Name, false, false);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("항목 {} 개 · 한 번 읽는 데 {ms:.1}ms · 60fps 면 초당 {:.0}ms", rows.len(), ms * 60.0);
        // 캐시를 두면 읽는 대신 **복제**가 남는다. 그것이 얼마나 싼지도 재 둔다 —
        // 복제가 읽기만큼 비싸면 캐시를 둔 뜻이 없다.
        let t = std::time::Instant::now();
        let mut n = 0usize;
        for _ in 0..100 {
            n += rows.clone().len();
        }
        let cms = t.elapsed().as_secs_f64() * 1000.0 / 100.0;
        println!("복제 한 번 {cms:.2}ms · 60fps 면 초당 {:.0}ms (항목 {n} 확인)", cms * 60.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_entries_hides_dotfiles() {
        use super::{read_entries, Sort};
        let dir = std::env::temp_dir().join(format!("nabi-hid-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(".secret"), b"x").unwrap();
        std::fs::write(dir.join("visible.txt"), b"x").unwrap();
        let shown = read_entries(&dir, Sort::Name, false, false);
        assert!(shown.iter().any(|r| r.name == "visible.txt"));
        assert!(!shown.iter().any(|r| r.name == ".secret"));
        let all = read_entries(&dir, Sort::Name, false, true);
        assert!(all.iter().any(|r| r.name == ".secret"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_hidden_dotfile_and_plain() {
        assert!(super::is_hidden(".bashrc", None)); // 유닉스식 점 파일.
        assert!(!super::is_hidden("readme.txt", None)); // 평범한 파일.
    }


    #[test]
    fn natural_cmp_numbers() {
        use super::natural_cmp;
        use std::cmp::Ordering;
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less); // 2 < 10.
        assert_eq!(natural_cmp("file10", "file2"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "B"), Ordering::Less); // 대소문자 무시.
        assert_eq!(natural_cmp("img02", "img2"), Ordering::Equal); // 선행 0.
        assert_eq!(natural_cmp("abc", "abc"), Ordering::Equal);
    }

    #[test]
    fn sort_u8_roundtrip() {
        use super::Sort;
        for s in [Sort::Name, Sort::Type, Sort::Size, Sort::Date] {
            assert_eq!(Sort::from_u8(s.to_u8()), s);
        }
        assert_eq!(Sort::from_u8(99), Sort::Name); // 잘못된 값 → 기본.
    }

    #[test]
    fn sort_next_cycles_all() {
        use super::Sort;
        // 정렬 토글이 4개 키를 모두 거쳐 처음으로 복귀해야 함(방향 순환이 이 동작에 의존).
        assert_eq!(Sort::Name.next(), Sort::Type);
        assert_eq!(Sort::Type.next(), Sort::Size);
        assert_eq!(Sort::Size.next(), Sort::Date);
        assert_eq!(Sort::Date.next(), Sort::Name);
    }

    #[test]
    fn sort_rows_modes() {
        use super::{sort_rows, Row, Sort};
        let mk = |name: &str, is_dir: bool, size: u64, mtime: u64| Row {
            name: name.into(),
            is_dir,
            is_link: false,
            size,
            mtime,
        };
        let mut v = vec![
            mk("b.txt", false, 10, 100),
            mk("dir", true, 0, 50),
            mk("a.txt", false, 99, 200),
        ];
        sort_rows(&mut v, Sort::Name, false); // 이름 오름차순.
        assert_eq!(v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["dir", "a.txt", "b.txt"]);
        sort_rows(&mut v, Sort::Size, false); // 폴더 먼저, 그다음 작은 파일.
        assert_eq!(v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["dir", "b.txt", "a.txt"]);
        sort_rows(&mut v, Sort::Size, true); // desc: 큰 파일 먼저.
        assert_eq!(v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["dir", "a.txt", "b.txt"]);
        sort_rows(&mut v, Sort::Date, false); // 오래된 것 먼저(100 < 200).
        assert_eq!(v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["dir", "b.txt", "a.txt"]);
    }


}
