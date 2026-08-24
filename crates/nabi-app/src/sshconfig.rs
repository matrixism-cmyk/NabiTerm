//! OpenSSH `~/.ssh/config` 파서 → 저장 세션. 기존 SSH 호스트를 목록으로 가져온다.

use nabi_session::{SavedSession, SessionKind};

/// `~/.ssh/config` 텍스트에서 Host 블록(별칭/HostName/User/Port/IdentityFile)을 추출한다.
/// 와일드카드 Host(*,?)는 건너뛴다. 폴더는 "ssh_config"로 묶는다.
pub(crate) fn parse_ssh_config(content: &str) -> Vec<SavedSession> {
    let mut out = Vec::new();
    let mut aliases: Vec<String> = Vec::new();
    let mut hostname = String::new();
    let mut user = String::new();
    let mut port: u16 = 22;
    let mut key: Option<String> = None;
    let mut jump: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // OpenSSH는 `Key Value`와 `Key=Value`(공백 선택)를 모두 허용한다.
        let split_at = line.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(line.len());
        let word = line[..split_at].to_ascii_lowercase();
        let val = line[split_at..].trim_start_matches([' ', '\t', '=']).trim().trim_matches('"');
        match word.as_str() {
            "host" => {
                flush(&mut out, &aliases, &hostname, &user, port, &key, &jump);
                // `Host a b c` — 한 블록의 여러 별칭을 각각 세션으로(와일드카드 토큰만 제외).
                aliases = val.split_whitespace().filter(|a| !a.contains(['*', '?'])).map(str::to_string).collect();
                hostname = String::new();
                user = String::new();
                port = 22;
                key = None;
                jump = None;
            }
            "match" => {
                // Match 블록은 조건부 설정 — 현재 Host를 종료해 이후 줄이 잘못 귀속되지 않게 한다.
                flush(&mut out, &aliases, &hostname, &user, port, &key, &jump);
                aliases = Vec::new();
            }
            "hostname" => hostname = val.to_string(),
            "user" => user = val.to_string(),
            "port" => port = val.parse().unwrap_or(22),
            "identityfile" => key = (!val.is_empty()).then(|| val.to_string()),
            // ProxyJump: 전체 체인 보존(다중 홉 a,b,c — QC 점프 필드가 콤마 멀티홉 지원). "none"은 무시.
            "proxyjump" => {
                let v = val.trim();
                jump = (!v.is_empty() && !v.eq_ignore_ascii_case("none")).then(|| v.to_string());
            }
            _ => {}
        }
    }
    flush(&mut out, &aliases, &hostname, &user, port, &key, &jump);
    out
}

/// 별칭(Host 이름)에 해당하는 (host, user, port, key)를 config 텍스트에서 찾는다.
/// 퀵커넥트 바에 별칭만 입력했을 때 HostName/User/Port를 채우는 데 쓴다.
pub(crate) fn resolve_alias(content: &str, alias: &str) -> Option<(String, String, u16, Option<String>)> {
    parse_ssh_config(content).into_iter().find(|s| s.name == alias).and_then(|s| match s.kind {
        SessionKind::Ssh { host, port, user, key_path, .. } => Some((host, user, port, key_path)),
        _ => None,
    })
}

/// 세션 목록을 OpenSSH `~/.ssh/config` 텍스트로 내보낸다(SSH 세션만, FTP 제외).
pub(crate) fn to_ssh_config(sessions: &[SavedSession]) -> String {
    let mut out = String::new();
    for s in sessions {
        let SessionKind::Ssh { host, port, user, key_path, jump, .. } = &s.kind else { continue };
        if s.is_ftp {
            continue;
        }
        out.push_str(&format!("Host {}\n    HostName {host}\n", s.name));
        if !user.is_empty() {
            out.push_str(&format!("    User {user}\n"));
        }
        if *port != 22 {
            out.push_str(&format!("    Port {port}\n"));
        }
        if let Some(k) = key_path.as_deref().filter(|k| !k.is_empty()) {
            out.push_str(&format!("    IdentityFile {k}\n"));
        }
        if let Some(j) = jump.as_deref().filter(|j| !j.is_empty()) {
            out.push_str(&format!("    ProxyJump {j}\n")); // 점프 호스트 보존(가져오기와 라운드트립).
        }
        out.push('\n');
    }
    out
}

/// 세션을 `ssh://user@host:port` URL로(권한부는 target_string SSOT 공유). SSH 세션이 아니면 None.
pub(crate) fn to_ssh_url(s: &SavedSession) -> Option<String> {
    if !matches!(s.kind, SessionKind::Ssh { .. }) {
        return None;
    }
    let scheme = if s.is_ftp { "sftp" } else { "ssh" };
    Some(format!("{scheme}://{}", s.target_string()))
}

#[allow(clippy::too_many_arguments)]
fn flush(
    out: &mut Vec<SavedSession>,
    aliases: &[String],
    hostname: &str,
    user: &str,
    port: u16,
    key: &Option<String>,
    jump: &Option<String>,
) {
    for a in aliases {
        // 별칭이 여럿이면 HostName이 없을 때 각자 자기 별칭을 호스트로 쓴다.
        let host = if hostname.is_empty() { a.clone() } else { hostname.to_string() };
        out.push(SavedSession {
            name: a.clone(),
            folder: Some("ssh_config".to_string()),
            kind: SessionKind::Ssh {
                host,
                port,
                user: user.to_string(),
                credential_ref: None,
                key_path: key.clone(),
                jump: jump.clone(),
            },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ssh_config;
    use nabi_session::SessionKind;

    #[test]
    fn parses_hosts_skips_wildcard() {
        let cfg = "Host web\n  HostName example.com\n  User alice\n  Port 2222\n\
                   Host *\n  User any\n\
                   Host db\n  HostName db.local\n";
        let s = parse_ssh_config(cfg);
        assert_eq!(s.len(), 2); // web, db — 와일드카드 제외
        assert_eq!(s[0].name, "web");
        match &s[0].kind {
            SessionKind::Ssh {
                host, port, user, ..
            } => {
                assert_eq!(host, "example.com");
                assert_eq!(*port, 2222);
                assert_eq!(user, "alice");
            }
            _ => panic!("expected ssh"),
        }
        // HostName 없으면 별칭을 호스트로.
        assert_eq!(s[1].name, "db");
        match &s[1].kind {
            SessionKind::Ssh { host, port, .. } => {
                assert_eq!(host, "db.local");
                assert_eq!(*port, 22);
            }
            _ => panic!("expected ssh"),
        }
    }

    #[test]
    fn export_roundtrips_via_parser() {
        // ProxyJump 멀티홉도 내보내기→가져오기 라운드트립으로 보존된다.
        let cfg = "Host web\n  HostName example.com\n  User alice\n  Port 2222\n  ProxyJump a,b\n";
        let parsed = parse_ssh_config(cfg);
        let out = super::to_ssh_config(&parsed);
        let back = parse_ssh_config(&out);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "web");
        match &back[0].kind {
            SessionKind::Ssh { host, port, user, jump, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("example.com", 2222, "alice"));
                assert_eq!(jump.as_deref(), Some("a,b")); // 점프 체인 보존.
            }
            _ => panic!(),
        }
    }

    #[test]
    fn resolve_alias_fills_fields() {
        let cfg = "Host web\n  HostName example.com\n  User alice\n  Port 2222\n";
        assert_eq!(
            super::resolve_alias(cfg, "web"),
            Some(("example.com".into(), "alice".into(), 2222, None))
        );
        assert_eq!(super::resolve_alias(cfg, "nope"), None); // 없는 별칭.
    }

    #[test]
    fn parses_multiple_aliases_per_host() {
        // `Host a b` — 한 블록이 두 별칭으로 확장(와일드카드 토큰 제외).
        let cfg = "Host web web2 *.dev\n  HostName example.com\n  User alice\n";
        let s = parse_ssh_config(cfg);
        assert_eq!(s.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["web", "web2"]);
        for e in &s {
            match &e.kind {
                SessionKind::Ssh { host, user, .. } => assert_eq!((host.as_str(), user.as_str()), ("example.com", "alice")),
                _ => panic!(),
            }
        }
    }

    #[test]
    fn parses_proxyjump() {
        let jof = |cfg: &str, i: usize| match &parse_ssh_config(cfg)[i].kind {
            SessionKind::Ssh { jump, .. } => jump.clone(),
            _ => None,
        };
        assert_eq!(jof("Host t\n HostName 10.0.0.5\n ProxyJump bastion.example.com\n", 0).as_deref(), Some("bastion.example.com"));
        assert_eq!(jof("Host t\n ProxyJump a,b,c\n", 0).as_deref(), Some("a,b,c")); // 다중 홉 체인 보존
        assert_eq!(jof("Host t2\n ProxyJump none\n", 0), None); // none=무시
    }

    #[test]
    fn session_to_command_and_url() {
        use nabi_session::SavedSession;
        let mk = |user: &str, port: u16| SavedSession {
            name: "x".into(),
            folder: None,
            kind: SessionKind::Ssh { host: "h".into(), port, user: user.into(), credential_ref: None, key_path: None, jump: None },
            on_connect: None,
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag: Default::default(),
        };
        assert_eq!(super::to_ssh_url(&mk("bob", 2222)).as_deref(), Some("ssh://bob@h:2222"));
        assert_eq!(super::to_ssh_url(&mk("", 22)).as_deref(), Some("ssh://h"));
    }

    #[test]
    fn match_block_ends_host() {
        // Match 이후 설정은 직전 Host에 귀속되지 않는다.
        let cfg = "Host web\n  HostName a.com\nMatch user bob\n  HostName wrong.com\n";
        let s = parse_ssh_config(cfg);
        assert_eq!(s.len(), 1);
        match &s[0].kind {
            SessionKind::Ssh { host, .. } => assert_eq!(host, "a.com"), // wrong.com 미반영.
            _ => panic!(),
        }
    }

    #[test]
    fn parses_equals_syntax() {
        // `Key=Value`(공백 선택) 형식도 파싱.
        let cfg = "Host=web\nHostName = example.com\nPort=2222\nUser =bob\n";
        let s = parse_ssh_config(cfg);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "web");
        match &s[0].kind {
            SessionKind::Ssh { host, port, user, .. } => {
                assert_eq!((host.as_str(), *port, user.as_str()), ("example.com", 2222, "bob"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parses_identityfile_and_default_port() {
        let cfg = "Host k\n  HostName k.io\n  IdentityFile ~/.ssh/id_ed25519\n";
        let s = parse_ssh_config(cfg);
        assert_eq!(s.len(), 1);
        match &s[0].kind {
            SessionKind::Ssh {
                port, key_path, ..
            } => {
                assert_eq!(*port, 22); // 포트 미지정 → 기본 22.
                assert_eq!(key_path.as_deref(), Some("~/.ssh/id_ed25519"));
            }
            _ => panic!("expected ssh"),
        }
    }
}
