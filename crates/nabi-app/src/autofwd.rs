//! **세션 연결 시 터널 자동 개통** — 늘 여는 포워딩을 매번 손으로 열지 않는다.
//!
//! DB나 관리 화면을 터널로 쓰는 사람은 그 서버에 붙을 때마다 같은 포워딩을 연다. 세션을
//! 저장해 두는 이유가 "매번 같은 것을 다시 치지 않기"인데 터널만 빠져 있었다.
//!
//! ## 왜 세션 파일이 아니라 설정에 두는가
//!
//! `SavedSession`에 필드를 더하는 것이 더 맞아 보이지만, 그 구조체를 만드는 자리가 서른
//! 곳이 넘고 그중 대부분이 필드를 하나하나 적는다. 배치 끝에서 서른 곳을 건드리는 것은
//! 얻는 것에 비해 위험이 크다.
//!
//! 그리고 선례가 있다 — `last_connected`도 **세션 이름을 키로 설정에** 산다. 세션에
//! 딸리지만 세션 파일에는 없는 것이 이미 있다는 뜻이다. 같은 방식을 따른다.
//!
//! ## 규칙 문법
//!
//! `로컬포트:원격호스트:원격포트` — ssh의 `-L`과 같은 순서다. 이미 아는 문법을 쓰는 편이
//! 새 문법을 배우게 하는 것보다 낫다.

/// 규칙 하나를 판다. 형식이 아니면 None.
pub(crate) fn parse_forward(spec: &str) -> Option<(u16, String, u16)> {
    let s = spec.trim();
    if s.is_empty() {
        return None;
    }
    let mut it = s.split(':');
    let local: u16 = it.next()?.trim().parse().ok()?;
    let host = it.next()?.trim().to_string();
    let remote: u16 = it.next()?.trim().parse().ok()?;
    // 넷째 조각이 있으면 형식이 아니다 — 조용히 앞 셋만 쓰면 오해를 남긴다.
    if it.next().is_some() || host.is_empty() || local == 0 || remote == 0 {
        return None;
    }
    Some((local, host, remote))
}

/// 한 세션의 규칙들을 판다. 형식이 아닌 줄은 버린다(하나가 틀렸다고 나머지를 막지 않는다).
pub(crate) fn parse_all(specs: &[String]) -> Vec<(u16, String, u16)> {
    specs.iter().filter_map(|s| parse_forward(s)).collect()
}

/// 사람이 읽는 한 줄(목록 표시용).
pub(crate) fn describe(local: u16, host: &str, remote: u16) -> String {
    format!("localhost:{local} \u{2192} {host}:{remote}")
}

impl crate::app::NabiApp {
    /// 이 세션에 걸어 둔 터널을 연다(연결할 때 한 번).
    ///
    /// SSH 세션에만 뜻이 있다 — 로컬 셸에는 통로가 없다. 이미 같은 규칙으로 열려 있으면
    /// 다시 열지 않는다(세션을 두 번 열면 포트가 이미 잡혀 있어 두 번째가 실패한다).
    pub(crate) fn start_auto_forwards(&mut self, session: &nabi_session::SavedSession) {
        if session.name.is_empty() {
            return;
        }
        let Some(specs) = self.config.terminal.auto_forwards.get(&session.name).cloned() else { return };
        let nabi_session::SessionKind::Ssh { host, port, user, .. } = &session.kind else { return };
        let (host, port, user) = (host.clone(), *port, user.clone());
        let mut opened = 0usize;
        for (local, rhost, rport) in crate::autofwd::parse_all(&specs) {
            let label = crate::autofwd::describe(local, &rhost, rport);
            if self.forward.active.iter().any(|(_, l)| l.ends_with(&label)) {
                continue; // 이미 열려 있다.
            }
            let id = self.next_fwd_id();
            // 비밀번호는 여기서 알 수 없다 — 볼트에 있으면 그것을, 없으면 빈 값으로 시도한다.
            // (키·에이전트 세션이면 빈 비밀번호로도 붙는다.)
            let pw = self.vault_get(&format!("ssh:{user}@{host}:{port}")).unwrap_or_default();
            let params = nabi_proto::SshParams::password(host.clone(), port, user.clone(), pw);
            self.orch.send(nabi_proto::Command::StartLocalForward { id, params, remote_host: rhost, remote_port: rport });
            self.forward.active.push((id, format!("{} {label}", session.name)));
            opened += 1;
        }
        if opened > 0 {
            self.notify = Some((
                format!("{} {opened}", nabi_i18n::tr(self.lang, "fwd.autoopened")),
                std::time::Instant::now(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ssh_dash_l_order_is_understood() {
        assert_eq!(parse_forward("5432:db.internal:5432"), Some((5432, "db.internal".into(), 5432)));
        assert_eq!(parse_forward(" 8080 : 127.0.0.1 : 80 "), Some((8080, "127.0.0.1".into(), 80)));
    }

    /// **형식이 아니면 조용히 절반만 쓰지 않는다** — 엉뚱한 포트로 터널이 열리면 더 나쁘다.
    #[test]
    fn malformed_rules_are_rejected_outright() {
        assert!(parse_forward("").is_none());
        assert!(parse_forward("5432").is_none());
        assert!(parse_forward("5432:db").is_none());
        assert!(parse_forward("5432:db:5432:extra").is_none(), "넷째 조각을 무시했다");
        assert!(parse_forward("abc:db:5432").is_none());
        assert!(parse_forward("5432::5432").is_none(), "빈 호스트를 받았다");
        assert!(parse_forward("0:db:5432").is_none(), "0번 포트를 받았다");
        assert!(parse_forward("5432:db:0").is_none());
    }

    /// 65535를 넘는 포트는 u16이 아니다.
    #[test]
    fn out_of_range_ports_are_rejected() {
        assert!(parse_forward("70000:db:5432").is_none());
    }

    /// 한 줄이 틀렸다고 나머지까지 막으면 사용자는 어느 줄이 문제인지 모른 채 전부를 잃는다.
    #[test]
    fn one_bad_line_does_not_stop_the_others() {
        let specs = vec!["5432:db:5432".to_string(), "쓰레기".to_string(), "8080:web:80".to_string()];
        let got = parse_all(&specs);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, 5432);
        assert_eq!(got[1].0, 8080);
    }

    #[test]
    fn a_rule_reads_the_way_it_works() {
        assert_eq!(describe(5432, "db", 5432), "localhost:5432 \u{2192} db:5432");
    }
}
