//! 렌더된 행에서 http(s):// URL과 절대 파일 경로를 인식한다(클릭형 하이퍼링크용). vt100 비의존.

use nabi_vt::RenderCell;

/// 행 안의 URL 한 개(셀 인덱스 범위[포함] + URL 문자열).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlSpan {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

/// 셀 하나를 (문자, 소유 셀 인덱스)로 평탄화해 버퍼에 넣는다.
/// 버퍼를 호출측이 들고 재사용할 수 있어 **셀 복제 없이** 여러 행을 이어 붙일 수 있다
/// (성능 리뷰 2026-08-19: screen_urls가 후보 그룹마다 RenderCell을 통째로 복제했다).
pub fn push_cell(cell: &RenderCell, index: usize, chars: &mut Vec<char>, owner: &mut Vec<usize>) {
    if cell.text.is_empty() {
        chars.push(' ');
        owner.push(index);
    } else {
        for c in cell.text.chars() {
            chars.push(c);
            owner.push(index);
        }
    }
}

/// 한 행에서 http(s):// URL들을 셀 인덱스 기준으로 찾는다.
pub fn row_urls(row: &[RenderCell]) -> Vec<UrlSpan> {
    let (mut chars, mut owner) = (Vec::new(), Vec::new());
    for (ci, cell) in row.iter().enumerate() {
        push_cell(cell, ci, &mut chars, &mut owner);
    }
    row_urls_from(&chars, &owner)
}

/// 평탄화된 (문자, 소유 셀 인덱스)에서 URL을 찾는다 — 반환 span은 **셀 인덱스** 기준.
pub fn row_urls_from(chars: &[char], owner: &[usize]) -> Vec<UrlSpan> {
    let mut spans = Vec::new();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let scheme = ["https://", "http://", "ftp://", "file://", "ssh://", "sftp://", "git://"]
            .iter()
            .find(|s| starts_with(chars, i, s));
        if let Some(s) = scheme {
            let mut j = i + s.chars().count();
            // 대괄호 IPv6 리터럴 호스트(`[::1]`)는 `]`까지 통째로 포함(is_url_char가 `]` 제외하므로).
            if chars.get(j) == Some(&'[') {
                while j < n && chars[j] != ']' { j += 1; }
                j = (j + 1).min(n); // 닫는 `]` 포함.
            }
            while j < n && is_url_char(chars[j]) { j += 1; }
            // 문장 끝 구두점은 URL에서 제외(예: "...x.com." → "...x.com").
            while j > i + 1 && matches!(chars[j - 1], '.' | ',' | ';' | ':' | '!' | '?') { j -= 1; }
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: chars[i..j].iter().collect() });
            i = j;
        } else if (i == 0 || chars[i - 1].is_whitespace()) && starts_with(chars, i, "mailto:") {
            // 이메일 하이퍼링크(mailto:user@host).
            let mut j = i + 7;
            while j < n && is_url_char(chars[j]) { j += 1; }
            while j > i + 1 && matches!(chars[j - 1], '.' | ',' | ';' | ':' | '!' | '?') { j -= 1; }
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: chars[i..j].iter().collect() });
            i = j;
        } else if (i == 0 || chars[i - 1].is_whitespace()) && starts_with(chars, i, "www.") {
            // 스킴 없는 www. 도메인: 열 때 https://를 보정.
            let mut j = i;
            while j < n && is_url_char(chars[j]) {
                j += 1;
            }
            // 문장 끝 구두점은 URL에서 제외(예: "...x.com." → "...x.com").
            while j > i + 1 && matches!(chars[j - 1], '.' | ',' | ';' | ':' | '!' | '?') {
                j -= 1;
            }
            let text: String = chars[i..j].iter().collect();
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: format!("https://{text}") });
            i = j;
        } else if let Some(hlen) = crate::urlspath::devhost_at(chars, i) {
            // 스킴 없는 로컬 개발 서버(localhost/loopback + :포트) → http:// 보정.
            let mut j = i + hlen;
            while j < n && is_url_char(chars[j]) {
                j += 1;
            }
            while j > i + 1 && matches!(chars[j - 1], '.' | ',' | ';' | ':' | '!' | '?') {
                j -= 1;
            }
            let text: String = chars[i..j].iter().collect();
            spans.push(UrlSpan {
                start: owner[i],
                end: owner[j - 1],
                url: format!("http://{text}"),
            });
            i = j;
        } else if let Some(j) = crate::urls_scp::scp_match(chars, i) {
            // scp/git 단축 주소(`git@host:path`) → ssh:// 링크. 경로에 `/`가 있어야 인정(포트 오탐 방지).
            let raw: String = chars[i..j].iter().collect();
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: crate::urls_scp::scp_to_ssh(&raw) });
            i = j;
        } else if let Some(j) = crate::urls_email::email_at(chars, i) {
            // 스킴 없는 이메일 주소(user@host.tld) → mailto: 링크(클릭=메일 클라이언트).
            let addr: String = chars[i..j].iter().collect();
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: format!("mailto:{addr}") });
            i = j;
        } else if let Some((j, url)) = crate::urls_pytrace::pytrace_at(chars, i) {
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url }); i = j; // Python 트레이스백→nabiPad.
        } else if let Some(j) = crate::fileref::file_ref_at(chars, i) {
            spans.push(UrlSpan { start: owner[i], end: owner[j - 1], url: chars[i..j].iter().collect() }); i = j; // 파일참조→nabiPad.
        } else if crate::urlspath::is_path_start(chars, i) {
            // 절대 파일 경로(드라이브 `C:\…`·`C:/…` 또는 UNC `\\server\…`).
            // Ctrl+클릭으로 OS 기본 앱에서 열린다. `:line` 참조는 경로에서 제외.
            let mut j = i + 2; // 드라이브/UNC 접두 다음부터 수집.
            while j < n && is_path_char(chars[j]) {
                j += 1;
            }
            while j > i + 2 && chars[j - 1] == '.' {
                j -= 1; // 문장 끝 마침표 제외.
            }
            j = crate::fileref::line_col_end(chars, j); // `:줄[:열]` 접미 포함(클릭 시 nabiPad 해당 줄).
            spans.push(UrlSpan {
                start: owner[i],
                end: owner[j - 1],
                url: chars[i..j].iter().collect(),
            });
            i = j;
        } else if crate::urlspath::is_unix_path_start(chars, i) {
            // 유닉스 절대경로(`/usr/...`) — SSH 리눅스 세션에서 흔하다. 오탐을 줄이려
            // 경계에서 시작 + 슬래시 2개 이상(`/a/b` 최소)일 때만 링크로 인정한다.
            let mut j = i + 1;
            while j < n && is_path_char(chars[j]) {
                j += 1;
            }
            while j > i + 1 && chars[j - 1] == '.' {
                j -= 1;
            }
            let text: String = chars[i..j].iter().collect();
            if text.matches('/').count() >= 2 {
                let je = crate::fileref::line_col_end(chars, j); // 확장자 없는 경로도 `:줄[:열]` 포함.
                spans.push(UrlSpan { start: owner[i], end: owner[je - 1], url: chars[i..je].iter().collect() });
                i = je;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    spans
}

/// 위치 i에서 스킴 없는 로컬 개발 서버(localhost/loopback + `:` + 숫자)면 호스트 길이를 돌려준다.
/// 경로에 포함되는 글자(공백·괄호·따옴표·`:`(line 참조)·`;`·`,`는 경계).
pub(crate) fn is_path_char(c: char) -> bool {
    !c.is_whitespace()
        && !matches!(
            c,
            '"' | '\'' | '<' | '>' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ':' | ';' | ','
        )
}

pub(crate) fn starts_with(chars: &[char], i: usize, pat: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    i + p.len() <= chars.len() && chars[i..i + p.len()] == p[..]
}

pub(crate) fn is_url_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '"' | '\'' | '<' | '>' | '`' | ')' | ']' | '}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_types::{CellAttrs, Rgba};

    fn cells(s: &str) -> Vec<RenderCell> {
        s.chars()
            .map(|c| RenderCell { text: c.to_string(), fg: Rgba::WHITE, bg: Rgba::BLACK, attrs: CellAttrs::default(), ul_color: None })
            .collect()
    }
    fn urls(s: &str) -> Vec<UrlSpan> {
        row_urls(&cells(s))
    }
    fn url1(s: &str) -> String {
        urls(s)[0].url.clone()
    }

    #[test]
    fn schemes_and_punctuation() {
        let s = urls("see https://a.bc/x end");
        assert_eq!((s[0].url.as_str(), s[0].start, s[0].end), ("https://a.bc/x", 4, 17));
        assert_eq!(url1("ssh://git@h:22/repo"), "ssh://git@h:22/repo");
        assert_eq!(url1("get ftp://h/f.txt now"), "ftp://h/f.txt");
        assert_eq!(url1("[https://a.bc]"), "https://a.bc"); // 닫는 괄호 제외.
        assert_eq!(url1("see https://a.bc/x. ok"), "https://a.bc/x"); // 끝 마침표 제외.
    }

    #[test]
    fn mailto_link() {
        assert_eq!(url1("contact mailto:me@x.com please"), "mailto:me@x.com");
    }

    #[test]
    fn bare_email() {
        assert_eq!(url1("contact me@x.com please"), "mailto:me@x.com"); // 맨 이메일 → mailto.
        assert!(urls("user@host words").is_empty()); // 점 없는 도메인 → 이메일 아님(ssh user@host 등).
    }

    #[test]
    fn ipv6_bracket_host() {
        assert_eq!(url1("open http://[::1]:8080/p here"), "http://[::1]:8080/p");
        assert_eq!(url1("ssh://[2001:db8::1]:22"), "ssh://[2001:db8::1]:22");
    }

    #[test]
    fn scp_git_link() {
        assert_eq!(url1("clone git@github.com:user/repo.git here"), "ssh://git@github.com/user/repo.git");
        assert_eq!(url1("scp u@h:/var/log/x ok"), "ssh://u@h//var/log/x");
        assert!(urls("send a@b:8080 now").is_empty()); // 경로에 / 없음(포트)·도메인 점 없음 → 무시.
        assert_eq!(url1("me@x.com words"), "mailto:me@x.com"); // 콜론 없는 맨 이메일 → mailto.
    }

    #[test]
    fn bare_www_and_devhost() {
        assert_eq!(url1("go www.example.com now"), "https://www.example.com");
        assert_eq!(url1("on localhost:3000 now"), "http://localhost:3000");
        assert_eq!(url1("see 127.0.0.1:8080/app ok"), "http://127.0.0.1:8080/app");
        assert!(urls("localhost alone").is_empty()); // 포트 없음.
        assert!(urls("mylocalhost:3000").is_empty()); // 경계 아님.
        assert_eq!(urls("http://localhost:3000").len(), 1); // 스킴 중복 없음.
    }

    #[test]
    fn windows_paths() {
        let s = urls("error C:\\proj\\main.rs found");
        assert_eq!((s[0].url.as_str(), s[0].start), ("C:\\proj\\main.rs", 6));
        assert_eq!(url1("at C:/a/b.rs:42:5 here"), "C:/a/b.rs:42:5"); // line 참조 포함(nabiPad 점프).
        assert_eq!(url1("open \\\\srv\\share\\f.txt now"), "\\\\srv\\share\\f.txt");
        assert!(urls("time 12:34 done").is_empty());
        assert!(urls("abcC:\\x").is_empty()); // 경계 아님.
    }

    #[test]
    fn unix_paths() {
        let s = urls("cd /var/log/syslog:42 now"); // 확장자 없어도 :줄 포함.
        assert_eq!((s[0].url.as_str(), s[0].start), ("/var/log/syslog:42", 3));
        assert_eq!(url1("/usr/bin/env x"), "/usr/bin/env"); // 줄 시작.
        assert_eq!(url1("see /a/b/c. ok"), "/a/b/c"); // 끝 마침표 제외.
        for bad in ["read/write mode", "ratio 12/2024 ok", "go /tmp now", "a // b"] {
            assert!(urls(bad).is_empty(), "{bad}");
        }
    }
}
