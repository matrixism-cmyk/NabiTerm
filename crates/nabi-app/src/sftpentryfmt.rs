//! SFTP/로컬 목록의 표시 유틸 — 양쪽 디렉터리 비교 상태와 권한(rwx) 문자열.
//! 전부 순수 함수라 테스트가 붙어 있다. 목록 렌더/입력은 sftpentries.

/// 디렉터리 비교 상태(상대편 기준).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Cmp {
    /// 상대편에 없음(이쪽에만).
    Missing,
    /// 양쪽에 있으나 크기 다름.
    Differ,
    /// 양쪽 동일(또는 디렉터리).
    Same,
}

/// (name,size,is_dir)을 상대편 맵(name→(is_dir,size))과 비교한 상태.
pub(crate) fn cmp_status(
    name: &str,
    size: u64,
    is_dir: bool,
    other: &std::collections::HashMap<String, (bool, u64)>,
) -> Cmp {
    match other.get(name) {
        None => Cmp::Missing,
        Some(&(od, os)) => {
            if is_dir || od || os == size {
                Cmp::Same
            } else {
                Cmp::Differ
            }
        }
    }
}

/// 비교 상태별 색(없으면 None=기본색).
pub(crate) fn cmp_color(c: Cmp) -> Option<egui::Color32> {
    match c {
        Cmp::Missing => Some(egui::Color32::from_rgb(0x5a, 0xc8, 0xfa)), // 하늘(이쪽에만)
        Cmp::Differ => Some(egui::Color32::from_rgb(0xff, 0xcc, 0x55)),  // 노랑(크기 다름)
        Cmp::Same => None,
    }
}

/// 8진 권한 모드를 `ls -l`식 rwx 문자열로 변환(예: 0o755 + dir → "drwxr-xr-x").
pub(crate) fn mode_to_rwx(mode: u32, is_dir: bool, is_link: bool) -> String {
    let mut s = String::with_capacity(10);
    s.push(if is_link { 'l' } else if is_dir { 'd' } else { '-' }); // ls -l 식 선두 종류 문자.
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0b111;
        s.push(if bits & 0b100 != 0 { 'r' } else { '-' });
        s.push(if bits & 0b010 != 0 { 'w' } else { '-' });
        s.push(if bits & 0b001 != 0 { 'x' } else { '-' });
    }
    s
}

/// 8진 권한 문자열을 파싱한다(예: "640", "0755"). 1~4자리·≤0o7777만 허용, 그 외 None.
pub(crate) fn parse_octal_mode(s: &str) -> Option<u32> {
    let t = s.trim();
    if t.is_empty() || t.len() > 4 || !t.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return None;
    }
    let m = u32::from_str_radix(t, 8).ok()?;
    (m <= 0o7777).then_some(m)
}

#[cfg(test)]
mod tests {
    use super::{cmp_status, mode_to_rwx, parse_octal_mode, Cmp};
    use std::collections::HashMap;

    #[test]
    fn parses_octal_mode() {
        assert_eq!(parse_octal_mode("640"), Some(0o640));
        assert_eq!(parse_octal_mode(" 0755 "), Some(0o755));
        assert_eq!(parse_octal_mode("7"), Some(0o7));
        assert_eq!(parse_octal_mode(""), None);
        assert_eq!(parse_octal_mode("8"), None); // 8진수 아님.
        assert_eq!(parse_octal_mode("99999"), None); // 너무 김.
        assert_eq!(parse_octal_mode("abc"), None);
    }

    #[test]
    fn rwx_formats_modes() {
        assert_eq!(mode_to_rwx(0o755, true, false), "drwxr-xr-x");
        assert_eq!(mode_to_rwx(0o644, false, false), "-rw-r--r--");
        assert_eq!(mode_to_rwx(0o600, false, false), "-rw-------");
        assert_eq!(mode_to_rwx(0o777, false, false), "-rwxrwxrwx");
        assert_eq!(mode_to_rwx(0, false, false), "----------");
        assert_eq!(mode_to_rwx(0o777, false, true), "lrwxrwxrwx"); // 심볼릭 링크는 'l'.
    }

    #[test]
    fn compare_status_classifies() {
        let mut other = HashMap::new();
        other.insert("a.txt".to_string(), (false, 10u64));
        other.insert("d".to_string(), (true, 0u64));
        assert_eq!(cmp_status("a.txt", 10, false, &other), Cmp::Same);
        assert_eq!(cmp_status("a.txt", 11, false, &other), Cmp::Differ);
        assert_eq!(cmp_status("b.txt", 1, false, &other), Cmp::Missing);
        assert_eq!(cmp_status("d", 0, true, &other), Cmp::Same, "디렉터리는 크기 비교 안 함");
    }
}
