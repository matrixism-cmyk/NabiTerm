//! scp/git 단축 주소(`user@host:path`) 인식 — 터미널에서 `git clone` 대상 등을 클릭형 링크로.
//! `urls.rs`에서 한 분기로 호출한다(파일 분할로 라인 한도 유지).

use crate::urls::is_url_char;

/// 위치 i에서 scp/git 단축 주소(`user@host:path`, path에 `/` 포함)가 시작하면 끝 인덱스를 돌려준다.
pub(crate) fn scp_match(chars: &[char], i: usize) -> Option<usize> {
    let boundary = i == 0 || chars[i - 1].is_whitespace() || matches!(chars[i - 1], '"' | '\'' | '(' | '[' | '=');
    if !boundary {
        return None;
    }
    let n = chars.len();
    let mut j = i;
    while j < n && is_userhost(chars[j]) {
        j += 1;
    }
    if j == i || chars.get(j) != Some(&'@') {
        return None;
    }
    j += 1;
    let host = j;
    while j < n && is_userhost(chars[j]) {
        j += 1;
    }
    if j == host || chars.get(j) != Some(&':') {
        return None;
    }
    j += 1;
    let path = j;
    while j < n && is_url_char(chars[j]) {
        j += 1;
    }
    while j > path + 1 && matches!(chars[j - 1], '.' | ',' | ';' | ':') {
        j -= 1;
    }
    let p: String = chars[path..j].iter().collect();
    (!p.is_empty() && p.contains('/')).then_some(j)
}

fn is_userhost(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// `user@host:path` → `ssh://user@host/path`.
pub(crate) fn scp_to_ssh(raw: &str) -> String {
    let (user, rest) = raw.split_once('@').unwrap_or(("", raw));
    let (host, path) = rest.split_once(':').unwrap_or((rest, ""));
    format!("ssh://{user}@{host}/{path}")
}

#[cfg(test)]
mod tests {
    use super::scp_to_ssh;

    #[test]
    fn rewrites_scp_to_ssh() {
        assert_eq!(scp_to_ssh("git@github.com:user/repo.git"), "ssh://git@github.com/user/repo.git");
    }
}
