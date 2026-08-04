//! 멀티홉 ProxyJump 체인 — 콤마로 구분된 점프 호스트들을 중첩 SshParams로 만든다(순수, 테스트).
//! "u@h1:22,u@h2" 처럼 여러 점프를 입력하면 첫 홉부터 차례로 경유한다(단일 홉은 기존과 동일).

use nabi_proto::{SshAuth, SshParams};

/// 콤마 구분 점프 문자열을 중첩 점프 체인으로. 각 홉은 같은 auth, user 미지정 시 default_user.
/// 반환된 체인의 최상위는 마지막 홉, 그 .jump가 안쪽(먼저 연결되는) 홉이다.
pub(crate) fn build_jumps(jump_str: &str, auth: &SshAuth, default_user: &str) -> Option<SshParams> {
    let mut chain: Option<SshParams> = None;
    for hop in jump_str.split(',').map(str::trim).filter(|h| !h.is_empty()) {
        let jp = crate::qcparse::parse_connect(hop)?;
        let user = jp.user.unwrap_or_else(|| default_user.to_string());
        let mut p = SshParams::password(jp.host, jp.port.unwrap_or(22), user, String::new());
        p.auth = auth.clone();
        if let Some(prev) = chain.take() {
            p.jump = Some(Box::new(prev));
        }
        chain = Some(p);
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::build_jumps;
    use nabi_proto::SshAuth;

    #[test]
    fn builds_nested_chain() {
        // "a,b" → 바깥=b, 안쪽=a(a가 먼저 연결됨).
        let c = build_jumps("a:22, b:23", &SshAuth::None, "u").unwrap();
        assert_eq!((c.host.as_str(), c.port), ("b", 23));
        let inner = c.jump.as_ref().unwrap();
        assert_eq!(inner.host, "a");
        assert!(inner.jump.is_none());
        // 단일 홉.
        let s = build_jumps("bastion", &SshAuth::None, "u").unwrap();
        assert_eq!((s.host.as_str(), s.user.as_str()), ("bastion", "u"));
        assert!(s.jump.is_none());
        // 빈 입력.
        assert!(build_jumps("  ,  ", &SshAuth::None, "u").is_none());
    }
}
