//! 빠른 연결 최근 호스트 기록(순수 로직) — "user@host:port" 문자열 목록 관리.

/// 최근 목록 맨 앞에 entry를 넣는다(중복 제거, 최대 max개로 컷). 빈 문자열은 무시.
pub(crate) fn push_recent(list: &mut Vec<String>, entry: String, max: usize) {
    if entry.trim().is_empty() {
        return;
    }
    list.retain(|e| e != &entry);
    list.insert(0, entry);
    list.truncate(max);
}

/// "user@host:port"를 (user, host, port)로 분해(기본 user="", 포트 22). 권한 파싱은 qcparse SSOT.
pub(crate) fn parse_recent(s: &str) -> (String, String, String) {
    match crate::qcparse::parse_target(s) {
        Some(p) => (p.user.unwrap_or_default(), p.host, p.port.map_or_else(|| "22".to_string(), |n| n.to_string())),
        None => (String::new(), s.to_string(), "22".to_string()),
    }
}

/// host 칸에 붙여넣은 "user@host[:port]"를 (user, host, port?)로 분리. '@' 없으면 None.
/// port는 ':'이 있을 때만 Some(덮어쓰기). 권한 파싱은 qcparse SSOT(대괄호 IPv6 일관).
pub(crate) fn split_target(input: &str) -> Option<(String, String, Option<String>)> {
    if !input.contains('@') {
        return None;
    }
    let p = crate::qcparse::parse_target(input)?;
    Some((p.user.unwrap_or_default(), p.host, p.port.map(|n| n.to_string())))
}

#[cfg(test)]
mod tests {
    use super::{parse_recent, push_recent, split_target};

    #[test]
    fn split_target_forms() {
        assert_eq!(
            split_target("root@h:2222"),
            Some(("root".into(), "h".into(), Some("2222".into())))
        );
        assert_eq!(
            split_target("u@host"),
            Some(("u".into(), "host".into(), None))
        );
        assert_eq!(split_target("plainhost"), None);
    }

    #[test]
    fn push_dedups_and_caps() {
        let mut l = vec!["a".to_string(), "b".to_string()];
        push_recent(&mut l, "b".into(), 3); // 중복 → 맨 앞으로.
        assert_eq!(l, vec!["b", "a"]);
        push_recent(&mut l, "c".into(), 2); // 컷 2.
        assert_eq!(l, vec!["c", "b"]);
        push_recent(&mut l, "  ".into(), 2); // 공백 무시.
        assert_eq!(l, vec!["c", "b"]);
    }

    #[test]
    fn parse_user_host_port() {
        assert_eq!(
            parse_recent("root@10.0.0.1:2222"),
            ("root".into(), "10.0.0.1".into(), "2222".into())
        );
        assert_eq!(parse_recent("host"), ("".into(), "host".into(), "22".into()));
        assert_eq!(parse_recent("a@b"), ("a".into(), "b".into(), "22".into()));
    }
}
