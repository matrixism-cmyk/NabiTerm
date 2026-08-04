//! 퀵커넥트 호스트 입력 파싱 — `user@host:port`, `host:port`, `ssh -p N user@host`,
//! `ssh://user@host:port/…`를 (user, host, port)로 분해한다. 앱 타입에 의존하지 않는
//! 순수 함수라 완전히 단위 테스트 가능하다(qcbar에서 필드 분배).

#[derive(Debug, PartialEq)]
pub(crate) struct Parsed {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// 점프 호스트(ProxyJump) — `ssh -J ...` 붙여넣기에서만 채워진다(그 외 None).
    pub jump: Option<String>,
}

/// 호스트 입력이 단순 호스트명이 아니라 "분해할 가치가 있는" 연결 문자열인가.
pub(crate) fn looks_full(s: &str) -> bool {
    let s = s.trim();
    s.contains('@')
        || s.contains("://")
        || s.contains(char::is_whitespace)
        || s.starts_with('[')
        || host_port(s).1.is_some()
}

/// 연결 문자열을 파싱한다. 호스트가 비면 None.
pub(crate) fn parse_connect(s: &str) -> Option<Parsed> {
    let s = s.trim();
    for sch in ["ssh://", "sftp://", "scp://", "telnet://"] {
        if let Some(rest) = s.strip_prefix(sch) {
            let target = rest.split(['/', '?']).next().unwrap_or("");
            return parse_target(target);
        }
    }
    // `telnet host [port]` 명령형.
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("telnet ") {
        let toks: Vec<&str> = s.split_whitespace().skip(1).filter(|t| !t.starts_with('-')).collect();
        let mut p = parse_target(toks.first()?)?;
        p.port = p.port.or_else(|| toks.get(1).and_then(|t| t.parse().ok()));
        return Some(p);
    }
    // `ssh`/`mosh [옵션] 대상` 명령형: -p(포트)/-l(사용자) 플래그와 대상 토큰 추출.
    if lower.starts_with("ssh ") || lower.starts_with("mosh ") {
        let (mut user, mut port, mut target, mut jump) = (None, None, None, None);
        let toks: Vec<&str> = s.split_whitespace().skip(1).collect();
        let mut i = 0;
        while i < toks.len() {
            match toks[i] {
                "-p" => { i += 1; port = toks.get(i).and_then(|t| t.parse().ok()); }
                "-l" => { i += 1; user = toks.get(i).map(|t| t.to_string()); }
                "-J" => { i += 1; jump = toks.get(i).map(|t| t.to_string()); } // 점프 호스트(ProxyJump) 포착.
                // 인자를 받는 다른 ssh 플래그(-i 키·-o 옵션·-F·-L/-R/-D 포워딩 등): 그 인자를
                // 호스트로 오인하지 않게 다음 토큰을 건너뛴다.
                "-i" | "-o" | "-F" | "-L" | "-R" | "-D" | "-W" | "-w" | "-b" | "-c" | "-m"
                | "-e" | "-E" | "-I" | "-O" | "-Q" | "-S" => i += 1,
                // 붙은형 단축 옵션(-p2222·-luser·-Jhost) — 값이 플래그에 붙은 형태도 포착.
                t if t.len() > 2 && t.starts_with("-p") => port = t[2..].parse().ok(),
                t if t.len() > 2 && t.starts_with("-l") => user = Some(t[2..].to_string()),
                t if t.len() > 2 && t.starts_with("-J") => jump = Some(t[2..].to_string()),
                t if t.starts_with('-') => {} // 인자 없는 플래그(-v/-A/-X/-t 등) 무시.
                t if target.is_none() => target = Some(t),
                _ => {}
            }
            i += 1;
        }
        let mut p = parse_target(target?)?;
        p.user = p.user.or(user);
        p.port = p.port.or(port);
        p.jump = jump;
        return Some(p);
    }
    parse_target(s)
}

/// `user@host:port`(또는 `[ipv6]:port`) 한 토큰을 분해. 권한 파싱 SSOT(connectsave도 위임).
pub(crate) fn parse_target(s: &str) -> Option<Parsed> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (user, rest) = match s.split_once('@') {
        // `user:pass@host`의 비밀번호는 버린다(세션에 비밀 저장 안 함).
        Some((u, r)) if !u.is_empty() => (Some(u.split(':').next().unwrap_or(u).to_string()), r),
        _ => (None, s),
    };
    let (host, port) = host_port(rest);
    (!host.is_empty()).then_some(Parsed { user, host, port, jump: None })
}

/// `[ipv6]:port` / `host:port` / `host`에서 (host, port?)를 뽑는다.
fn host_port(s: &str) -> (String, Option<u16>) {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some((h, after)) = rest.split_once(']') {
            let port = after.strip_prefix(':').and_then(|p| p.parse().ok());
            return (h.to_string(), port);
        }
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            (h.to_string(), p.parse().ok())
        }
        _ => (s.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(u: Option<&str>, h: &str, port: Option<u16>) -> Parsed {
        Parsed { user: u.map(str::to_string), host: h.to_string(), port, jump: None }
    }

    #[test]
    fn plain_and_userhost() {
        assert_eq!(parse_connect("host"), Some(p(None, "host", None)));
        assert_eq!(parse_connect("me@host"), Some(p(Some("me"), "host", None)));
        assert_eq!(parse_connect("me@host:2222"), Some(p(Some("me"), "host", Some(2222))));
        assert_eq!(parse_connect("host:22"), Some(p(None, "host", Some(22))));
    }

    #[test]
    fn ssh_command_forms() {
        assert_eq!(parse_connect("ssh -p 2222 me@host"), Some(p(Some("me"), "host", Some(2222))));
        assert_eq!(parse_connect("ssh me@host -p 22"), Some(p(Some("me"), "host", Some(22))));
        assert_eq!(parse_connect("ssh -l me host"), Some(p(Some("me"), "host", None)));
        // 인자 받는 플래그의 인자를 호스트로 오인하지 않는다(-i 키·-J 점프·-o 옵션).
        assert_eq!(parse_connect("ssh -i mykey.pem me@host"), Some(p(Some("me"), "host", None)));
        // -J 점프 호스트는 포착되어 jump에 채워진다(나머지는 동일).
        let j = parse_connect("ssh -J bastion -p 22 me@host").unwrap();
        assert_eq!((j.user.as_deref(), j.host.as_str(), j.port, j.jump.as_deref()), (Some("me"), "host", Some(22), Some("bastion")));
        assert_eq!(parse_connect("ssh -o StrictHostKeyChecking=no host"), Some(p(None, "host", None)));
        // 붙은형 단축 옵션(-p2222·-lme)도 분해.
        assert_eq!(parse_connect("ssh -p2222 -lme host"), Some(p(Some("me"), "host", Some(2222))));
        assert_eq!(parse_connect("ssh://me@host:2200/x"), Some(p(Some("me"), "host", Some(2200))));
        assert_eq!(parse_connect("sftp://u@h:22"), Some(p(Some("u"), "h", Some(22))));
        assert_eq!(parse_connect("scp://h"), Some(p(None, "h", None)));
    }

    #[test]
    fn password_and_telnet_mosh() {
        assert_eq!(parse_connect("user:secret@host:22"), Some(p(Some("user"), "host", Some(22)))); // 비번 제거.
        assert_eq!(parse_connect("telnet host 2323"), Some(p(None, "host", Some(2323))));
        assert_eq!(parse_connect("telnet://h:23"), Some(p(None, "h", Some(23))));
        assert_eq!(parse_connect("mosh me@host"), Some(p(Some("me"), "host", None)));
    }

    #[test]
    fn ipv6_bracketed() {
        assert_eq!(parse_connect("[::1]:22"), Some(p(None, "::1", Some(22))));
    }

    #[test]
    fn looks_full_gate() {
        assert!(!looks_full("host"));
        assert!(!looks_full("192.168.0.1"));
        assert!(looks_full("me@host"));
        assert!(looks_full("host:22"));
        assert!(looks_full("ssh -p 22 host"));
    }
}
