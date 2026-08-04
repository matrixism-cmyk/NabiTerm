//! `known_hosts` 한 줄 파싱/매칭(평문 항목) — known_hosts 관리 UI·중복 검사용.
//! 해시(`|1|...`)·와일드카드 항목은 평문 매칭 불가라 건너뛴다(파싱 None).

/// known_hosts 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHostEntry {
    /// 쉼표로 나뉜 호스트 패턴들(`h`, `[h]:port`, `h1,h2`).
    pub hosts: Vec<String>,
    /// 키 타입(예: `ssh-ed25519`, `ecdsa-sha2-nistp256`).
    pub key_type: String,
    /// base64 공개키.
    pub key_b64: String,
}

/// `host[,host2] keytype base64 [comment]` 한 줄을 파싱한다. 주석·해시·불완전 줄은 None.
pub fn parse_known_hosts_line(line: &str) -> Option<KnownHostEntry> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut it = line.split_whitespace();
    let host_field = it.next()?;
    if host_field.starts_with("|1|") {
        return None; // 해시 항목은 평문으로 매칭할 수 없다.
    }
    // `@cert-authority`/`@revoked` 마커가 앞에 오면 다음 필드가 호스트.
    let (host_field, key_type) = if host_field.starts_with('@') {
        (it.next()?, it.next()?)
    } else {
        (host_field, it.next()?)
    };
    let key_b64 = it.next()?.to_string();
    Some(KnownHostEntry {
        hosts: host_field.split(',').map(str::to_string).collect(),
        key_type: key_type.to_string(),
        key_b64,
    })
}

/// 항목이 주어진 host/port에 해당하는가(22는 평문 호스트, 그 외는 `[host]:port`, 대소문자 무시).
pub fn known_hosts_match(entry: &KnownHostEntry, host: &str, port: u16) -> bool {
    let needle = if port == 22 { host.to_string() } else { format!("[{host}]:{port}") };
    entry.hosts.iter().any(|h| h.eq_ignore_ascii_case(&needle))
}

impl KnownHostEntry {
    /// known_hosts 한 줄로 직렬화(`host[,host2] keytype base64`). [`parse_known_hosts_line`]와 왕복.
    pub fn to_line(&self) -> String {
        format!("{} {} {}", self.hosts.join(","), self.key_type, self.key_b64)
    }
}

/// known_hosts 내용에서 host/port에 해당하는 첫 항목을 찾는다(없으면 None).
pub fn known_hosts_find(content: &str, host: &str, port: u16) -> Option<KnownHostEntry> {
    content.lines().filter_map(parse_known_hosts_line).find(|e| known_hosts_match(e, host, port))
}

/// known_hosts 내용에서 host/port에 매칭되는 항목 줄을 모두 제거한다(주석·빈 줄·비매칭은 보존).
pub fn known_hosts_remove(content: &str, host: &str, port: u16) -> String {
    content
        .lines()
        .filter(|l| match parse_known_hosts_line(l) {
            Some(e) => !known_hosts_match(&e, host, port),
            None => true, // 주석/빈 줄/해시 항목은 그대로 유지.
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// known_hosts 내용에서 완전히 동일한 항목 줄을 제거한다(빈 줄·주석은 보존, 순서 유지).
pub fn known_hosts_dedupe(content: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    content
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('#') || seen.insert(t.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_matches() {
        let e = parse_known_hosts_line("example.com,[example.com]:2222 ssh-ed25519 AAAAC3Nz comment").unwrap();
        assert_eq!(e.key_type, "ssh-ed25519");
        assert_eq!(e.key_b64, "AAAAC3Nz");
        assert!(known_hosts_match(&e, "example.com", 22));
        assert!(known_hosts_match(&e, "EXAMPLE.COM", 2222)); // [host]:port + 대소문자.
        assert!(!known_hosts_match(&e, "other.com", 22));
    }

    #[test]
    fn to_line_roundtrips_and_find_dedupe() {
        let e = parse_known_hosts_line("h.com ssh-ed25519 AAAAB3").unwrap();
        assert_eq!(e.to_line(), "h.com ssh-ed25519 AAAAB3");
        let content = "# hosts\nh.com ssh-ed25519 AAAA\nh.com ssh-ed25519 AAAA\n[h.com]:2222 ssh-rsa BBBB\n";
        assert!(known_hosts_find(content, "h.com", 22).is_some());
        assert!(known_hosts_find(content, "h.com", 2222).is_some());
        assert!(known_hosts_find(content, "nope", 22).is_none());
        // 동일 항목 1줄 제거(주석 유지).
        let deduped = known_hosts_dedupe(content);
        assert_eq!(deduped.matches("h.com ssh-ed25519 AAAA").count(), 1);
        assert!(deduped.contains("# hosts"));
        // host 매칭 제거(22 항목 제거, [h.com]:2222·주석 보존).
        let removed = known_hosts_remove(content, "h.com", 22);
        assert!(!removed.contains("h.com ssh-ed25519"));
        assert!(removed.contains("[h.com]:2222"));
        assert!(removed.contains("# hosts"));
    }

    #[test]
    fn skips_comments_hashed_incomplete() {
        assert!(parse_known_hosts_line("# comment").is_none());
        assert!(parse_known_hosts_line("|1|abc|def ssh-rsa AAAA").is_none()); // 해시.
        assert!(parse_known_hosts_line("hostonly").is_none()); // 키 없음.
        // @marker 접두 처리.
        let e = parse_known_hosts_line("@cert-authority *.x.com ssh-rsa AAAB").unwrap();
        assert_eq!(e.key_type, "ssh-rsa");
    }
}
