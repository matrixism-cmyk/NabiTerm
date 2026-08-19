//! URL 감지의 경로/호스트 판정 헬퍼 — urls.rs에서 분리(라인 한도).
//!
//! 윈도/유닉스 경로 시작과 "dev 호스트"(localhost:3000 같은 스킴 없는 주소) 판정.

pub(crate) fn devhost_at(chars: &[char], i: usize) -> Option<usize> {
    let boundary = i == 0
        || chars[i - 1].is_whitespace()
        || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '=');
    if !boundary {
        return None;
    }
    for host in ["localhost", "127.0.0.1", "0.0.0.0"] {
        let hl = host.chars().count();
        if crate::urls::starts_with(chars, i, host)
            && chars.get(i + hl) == Some(&':')
            && chars.get(i + hl + 1).is_some_and(|c| c.is_ascii_digit())
        {
            return Some(hl);
        }
    }
    None
}

/// 위치 i에서 절대 경로가 시작하는가(앞이 경계이고 드라이브/UNC 접두).
pub(crate) fn is_path_start(chars: &[char], i: usize) -> bool {
    let boundary = i == 0
        || chars[i - 1].is_whitespace()
        || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '=');
    if !boundary {
        return false;
    }
    let n = chars.len();
    // 드라이브 경로: [A-Za-z] ':' ('\\' | '/').
    let drive = i + 2 < n
        && chars[i].is_ascii_alphabetic()
        && chars[i + 1] == ':'
        && matches!(chars[i + 2], '\\' | '/');
    // UNC 경로: '\\' '\\'.
    let unc = i + 2 < n && chars[i] == '\\' && chars[i + 1] == '\\';
    drive || unc
}

/// 위치 i에서 유닉스 절대경로가 시작하는가(앞이 경계이고 `/` 다음이 이름 글자).
/// `//`·`/ `(슬래시 뒤 비이름)은 제외 — 나눗셈/주석 등 오탐 방지.
pub(crate) fn is_unix_path_start(chars: &[char], i: usize) -> bool {
    let boundary = i == 0
        || chars[i - 1].is_whitespace()
        || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '=');
    boundary
        && chars.get(i) == Some(&'/')
        && chars.get(i + 1).is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '~'))
}
