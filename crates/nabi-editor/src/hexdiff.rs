//! **이진 파일 비교** — 두 파일이 처음 갈리는 자리를 찾는다.
//!
//! 글 파일 비교는 세 갈래가 있다(브라우저의 두 파일·디스크 원본·열린 두 문서). 그런데
//! 이진 파일은 줄이 없어 그 방법이 통하지 않는다. 펌웨어·인코딩·전송 무결성을 볼 때
//! 알고 싶은 것은 "어디서부터 다른가"이고, 그건 줄이 아니라 **바이트 자리**다.
//!
//! ## 왜 전부 나열하지 않는가
//!
//! 몇 MB짜리 두 파일이 앞에서부터 다르면 차이가 수백만 개다. 그걸 다 보여 주는 것은
//! 보여 주지 않는 것과 같다. 그래서 **처음 갈리는 자리**와 앞쪽 몇 군데만 짚고, 몇 개를
//! 생략했는지 말한다.

/// 다른 곳 한 군데.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Diff {
    /// 파일 처음부터의 바이트 자리.
    pub at: u64,
    /// 왼쪽 값(짧아서 없으면 None).
    pub a: Option<u8>,
    /// 오른쪽 값.
    pub b: Option<u8>,
}

/// 비교 결과.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HexDiff {
    /// 같은가.
    pub same: bool,
    /// 두 파일 길이.
    pub len_a: u64,
    pub len_b: u64,
    /// 앞쪽에서 찾은 차이들(상한까지).
    pub diffs: Vec<Diff>,
    /// 상한을 넘겨 보여 주지 못한 개수(적어도 이만큼 더 있다).
    pub more: u64,
}

/// 몇 군데까지 짚을 것인가.
pub const MAX_DIFFS: usize = 200;

/// 두 바이트 열을 견준다.
pub fn compare(a: &[u8], b: &[u8]) -> HexDiff {
    let n = a.len().max(b.len());
    let mut diffs = Vec::new();
    let mut more = 0u64;
    for i in 0..n {
        let (x, y) = (a.get(i).copied(), b.get(i).copied());
        if x == y {
            continue;
        }
        if diffs.len() < MAX_DIFFS {
            diffs.push(Diff { at: i as u64, a: x, b: y });
        } else {
            more += 1;
        }
    }
    HexDiff {
        same: diffs.is_empty() && more == 0,
        len_a: a.len() as u64,
        len_b: b.len() as u64,
        diffs,
        more,
    }
}

/// 처음 갈리는 자리(같으면 None).
pub fn first_difference(d: &HexDiff) -> Option<u64> {
    d.diffs.first().map(|x| x.at)
}

/// 한 줄로 요약. 화면 맨 위에 적는다.
pub fn summary(d: &HexDiff) -> String {
    if d.same {
        return "same".to_string();
    }
    let head = match first_difference(d) {
        Some(at) => format!("first difference at 0x{at:08x}"),
        None => "sizes differ".to_string(),
    };
    let total = d.diffs.len() as u64 + d.more;
    format!("{head} · {total} byte(s) differ · {} / {}", d.len_a, d.len_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_files_are_same() {
        let d = compare(b"hello", b"hello");
        assert!(d.same);
        assert!(first_difference(&d).is_none());
        assert_eq!(summary(&d), "same");
    }

    #[test]
    fn the_first_difference_is_found() {
        let d = compare(b"hello", b"heLlo");
        assert!(!d.same);
        assert_eq!(first_difference(&d), Some(2));
        assert_eq!(d.diffs[0], Diff { at: 2, a: Some(b'l'), b: Some(b'L') });
    }

    /// **길이가 다르면 남는 쪽이 전부 차이다** — 짧은 쪽은 값이 없다.
    #[test]
    fn a_longer_file_differs_in_its_tail() {
        let d = compare(b"abc", b"abcdef");
        assert_eq!(d.diffs.len(), 3);
        assert_eq!(d.diffs[0], Diff { at: 3, a: None, b: Some(b'd') });
        assert_eq!((d.len_a, d.len_b), (3, 6));
    }

    /// 빈 파일과 빈 파일은 같다.
    #[test]
    fn two_empty_files_are_same() {
        assert!(compare(b"", b"").same);
    }

    #[test]
    fn an_empty_file_differs_from_a_nonempty_one() {
        let d = compare(b"", b"x");
        assert!(!d.same);
        assert_eq!(first_difference(&d), Some(0));
    }

    /// **상한을 넘으면 몇 개를 생략했는지 말한다** — 조용히 자르면 "이게 전부"로 읽힌다.
    #[test]
    fn truncation_is_reported() {
        let a = vec![0u8; MAX_DIFFS + 50];
        let b = vec![1u8; MAX_DIFFS + 50];
        let d = compare(&a, &b);
        assert_eq!(d.diffs.len(), MAX_DIFFS);
        assert_eq!(d.more, 50);
        assert!(summary(&d).contains(&format!("{} byte(s)", MAX_DIFFS + 50)));
    }

    /// 요약에 처음 갈리는 자리가 16진으로 들어가야 헥스 화면에서 바로 찾아간다.
    #[test]
    fn the_summary_points_at_the_first_difference() {
        let mut a = vec![0u8; 300];
        let mut b = a.clone();
        b[0x101] = 9;
        a[0x101] = 8;
        let s = summary(&compare(&a, &b));
        assert!(s.contains("0x00000101"), "{s}");
    }
}
