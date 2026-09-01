//! 멀티홉 ProxyJump 체인 — 콤마로 구분된 점프 호스트들을 중첩 SshParams로 만든다(순수, 테스트).
//! "u@h1:22,u@h2" 처럼 여러 점프를 입력하면 첫 홉부터 차례로 경유한다(단일 홉은 기존과 동일).

use nabi_proto::{SshAuth, SshParams};

/// 점프 문자열을 읽은 결과.
///
/// 예전에는 `Option` 하나였다 — 그래서 **"안 적었다"와 "적었는데 틀렸다"가 같은 답**이었고,
/// 부르는 쪽은 둘 다 `None => params`, 즉 *점프 없이 대상에 직접 연결*로 처리했다.
/// 베스천을 반드시 거쳐야 하는 곳에서 오타 하나면 조용히 다른 길로 나가는 셈이다.
/// 갈래를 나눠 두면 컴파일러가 부르는 쪽마다 "틀렸을 때는 어쩔 건가"를 묻는다.
pub(crate) enum Jumps {
    /// 점프를 적지 않았다 — 대상에 곧장 붙는 것이 맞다.
    None,
    /// 정상 체인. 최상위가 마지막 홉이고 그 `.jump` 가 먼저 연결되는 홉이다.
    Chain(Box<SshParams>),
    /// 적었는데 모양이 틀렸다. 담긴 글은 **문제가 된 그 토막**이다(사람에게 보여 준다).
    Invalid(String),
}

/// 콤마 구분 점프 문자열을 중첩 점프 체인으로. 각 홉은 같은 auth, user 미지정 시 default_user.
///
/// 호스트·사용자 이름은 `sshsafe` 로 검사한다. `-oProxyCommand=...` 처럼 이름이 아니라
/// 옵션으로 읽히는 것, 줄바꿈이 들어 설정 파일에 새 줄을 만드는 것을 여기서 막는다
/// (OpenSSH 10.3 이 같은 자리를 막았다 — 2026-09-01 조사).
pub(crate) fn build_jumps(jump_str: &str, auth: &SshAuth, default_user: &str) -> Jumps {
    let mut chain: Option<SshParams> = None;
    for hop in jump_str.split(',').map(str::trim).filter(|h| !h.is_empty()) {
        let Some(jp) = crate::qcparse::parse_connect(hop) else {
            return Jumps::Invalid(hop.to_string());
        };
        let user = jp.user.unwrap_or_else(|| default_user.to_string());
        if !crate::sshsafe::valid_host(&jp.host) || !crate::sshsafe::valid_user(&user) {
            return Jumps::Invalid(hop.to_string());
        }
        let mut p = SshParams::password(jp.host, jp.port.unwrap_or(22), user, String::new());
        p.auth = auth.clone();
        if let Some(prev) = chain.take() {
            p.jump = Some(Box::new(prev));
        }
        chain = Some(p);
    }
    match chain {
        Some(c) => Jumps::Chain(Box::new(c)),
        None => Jumps::None,
    }
}

impl crate::app::NabiApp {
    /// 점프 호스트가 틀렸다고 알린다 — **연결은 하지 않았다**는 뜻이 함께 전해져야 한다.
    pub(crate) fn notify_jump_error(&mut self, bad: &str) {
        let msg = format!("{} {bad}", nabi_i18n::tr(self.lang, "net.jump.bad"));
        self.notify = Some((msg, std::time::Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::{build_jumps, Jumps};
    use nabi_proto::SshAuth;

    fn chain(s: &str) -> Option<nabi_proto::SshParams> {
        match build_jumps(s, &SshAuth::None, "u") {
            Jumps::Chain(c) => Some(*c),
            _ => None,
        }
    }

    #[test]
    fn builds_nested_chain() {
        // "a,b" → 바깥=b, 안쪽=a(a가 먼저 연결됨).
        let c = chain("a:22, b:23").unwrap();
        assert_eq!((c.host.as_str(), c.port), ("b", 23));
        let inner = c.jump.as_ref().unwrap();
        assert_eq!(inner.host, "a");
        assert!(inner.jump.is_none());
        // 단일 홉.
        let s = chain("bastion").unwrap();
        assert_eq!((s.host.as_str(), s.user.as_str()), ("bastion", "u"));
        assert!(s.jump.is_none());
    }

    /// **적지 않은 것과 틀리게 적은 것은 다른 답이어야 한다.**
    ///
    /// 예전에는 둘 다 `None` 이라, 오타가 나면 베스천을 건너뛰고 대상에 직접 붙었다.
    #[test]
    fn nothing_typed_and_typed_wrong_are_different_answers() {
        assert!(matches!(build_jumps("  ,  ", &SshAuth::None, "u"), Jumps::None));
        assert!(matches!(build_jumps("", &SshAuth::None, "u"), Jumps::None));
        for bad in ["-oProxyCommand=calc", "host\nProxyCommand calc", "h;calc"] {
            match build_jumps(bad, &SshAuth::None, "u") {
                Jumps::Invalid(t) => assert_eq!(t, bad, "문제가 된 토막을 그대로 돌려줘야 한다"),
                _ => panic!("{bad:?} 는 Invalid 여야 한다"),
            }
        }
    }

    /// 여러 홉 중 **하나만** 틀려도 전체를 물린다 — 나머지로 대충 붙이면 그것도 다른 길이다.
    #[test]
    fn one_bad_hop_rejects_the_whole_chain() {
        match build_jumps("good1, -oX=1, good2", &SshAuth::None, "u") {
            Jumps::Invalid(t) => assert_eq!(t, "-oX=1"),
            _ => panic!("가운데 홉이 틀리면 Invalid 여야 한다"),
        }
    }
}
