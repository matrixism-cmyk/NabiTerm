//! **세션 도달성 일괄 확인** — 붙기 전에 어느 서버가 지금 살아 있는지 안다.
//!
//! 하나짜리 확인은 있었다(`test_connection`, 세션 우클릭). 그런데 서버가 열 개인 폴더에서
//! 열 번 우클릭하게 하는 것은 도구가 할 일이 아니다. 한 번에 훑고 **목록에 표시**한다.
//!
//! ## 왜 TCP만 두드리는가
//!
//! SSH 인증까지 해 보면 확실하지만, 여러 서버에 동시에 로그인을 시도하는 것은
//! 위험하다 — 실패가 쌓이면 fail2ban류에 막히고, 비밀번호 세션은 볼트를 열어야 한다.
//! "포트가 열려 있는가"는 훨씬 싸고, 접속 전에 알고 싶은 것의 대부분을 답한다.
//! 그래서 **그 이상은 하지 않는다**고 화면에도 적는다.

/// 한 서버의 확인 결과.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Reach {
    /// 아직 확인 중.
    Checking,
    /// 포트가 열려 있다.
    Open,
    /// 닫혀 있거나 이름을 풀지 못했다.
    Closed,
}

impl Reach {
    /// 목록에 붙일 표식.
    pub fn mark(self) -> &'static str {
        match self {
            Reach::Checking => "\u{22ef}",
            Reach::Open => "\u{25cf}",
            Reach::Closed => "\u{25cb}",
        }
    }

    /// 표식 색.
    pub fn color(self) -> egui::Color32 {
        match self {
            Reach::Checking => egui::Color32::GRAY,
            Reach::Open => egui::Color32::from_rgb(0x3c, 0xa8, 0x55),
            Reach::Closed => egui::Color32::from_rgb(0xd0, 0x4a, 0x3a),
        }
    }
}

/// 한 번에 몇 개까지 동시에 두드릴 것인가.
///
/// 너무 많이 한꺼번에 열면 방화벽·NAT 표가 터진다. 스무 개면 열 개짜리 폴더는 한 번에,
/// 백 개짜리도 몇 물결이면 끝난다.
pub(crate) const PARALLEL: usize = 20;

/// 한 서버에 얼마나 기다릴 것인가. 길면 목록 전체가 늦어지고, 짧으면 느린 망에서 오판한다.
pub(crate) const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// 확인할 대상 목록을 만든다 — SSH 세션만, 같은 (호스트,포트)는 한 번만.
///
/// 같은 서버를 가리키는 세션이 여럿인 경우가 흔하다(다른 계정·다른 폴더). 그때마다 두드리면
/// 같은 서버를 여러 번 때리게 된다.
pub(crate) fn targets(sessions: &[(String, String, u16)]) -> Vec<(String, u16)> {
    let mut out: Vec<(String, u16)> = Vec::new();
    for (_, host, port) in sessions {
        if host.is_empty() {
            continue;
        }
        if !out.iter().any(|(h, p)| h == host && p == port) {
            out.push((host.clone(), *port));
        }
    }
    out
}

impl crate::app::NabiApp {
    /// 저장된 SSH 세션들의 포트가 지금 열려 있는지 한 번에 훑는다.
    ///
    /// 스레드를 세션 수만큼 만들지 않는다 — 백 개짜리 목록에서 백 개의 스레드는
    /// 확인이 아니라 부하다. 정해진 수만큼만 동시에 돈다.
    pub(crate) fn check_all_reachable(&mut self, ctx: &egui::Context) {
        let list: Vec<(String, String, u16)> = self
            .sessions
            .sessions
            .iter()
            .filter_map(|s| match &s.kind {
                nabi_session::SessionKind::Ssh { host, port, .. } => {
                    Some((s.name.clone(), host.clone(), *port))
                }
                _ => None,
            })
            .collect();
        let targets = crate::reachall::targets(&list);
        if targets.is_empty() {
            self.notify = Some((nabi_i18n::tr(self.lang, "reach.none").to_string(), std::time::Instant::now()));
            return;
        }
        {
            let mut m = self.reach_all.lock().unwrap_or_else(|e| e.into_inner());
            m.clear();
            for t in &targets {
                m.insert(t.clone(), crate::reachall::Reach::Checking);
            }
        }
        let (store, ctx2) = (self.reach_all.clone(), ctx.clone());
        std::thread::spawn(move || {
            for wave in targets.chunks(crate::reachall::PARALLEL) {
                let hands: Vec<_> = wave
                    .iter()
                    .cloned()
                    .map(|(h, p)| {
                        let s = store.clone();
                        std::thread::spawn(move || {
                            let ok = probe(&h, p);
                            let st = match ok {
                                true => crate::reachall::Reach::Open,
                                false => crate::reachall::Reach::Closed,
                            };
                            s.lock().unwrap_or_else(|e| e.into_inner()).insert((h, p), st);
                        })
                    })
                    .collect();
                for h in hands {
                    let _ = h.join();
                }
                ctx2.request_repaint(); // 물결마다 화면을 갱신해 진행이 보이게.
            }
        });
    }

}

/// 포트가 열려 있는가. 이름 풀이 실패도 "닫힘"으로 본다(사용자에게는 같은 뜻이다).
fn probe(host: &str, port: u16) -> bool {
    use std::net::ToSocketAddrs;
    format!("{host}:{port}")
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .map(|addr| std::net::TcpStream::connect_timeout(&addr, crate::reachall::TIMEOUT).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_server_is_probed_once() {
        let s = vec![
            ("a".into(), "h1".to_string(), 22u16),
            ("b".into(), "h1".to_string(), 22u16), // 같은 서버, 다른 계정.
            ("c".into(), "h2".to_string(), 22u16),
            ("d".into(), "h1".to_string(), 2222u16), // 같은 호스트, 다른 포트.
        ];
        let t = targets(&s);
        assert_eq!(t.len(), 3, "{t:?}");
        assert!(t.contains(&("h1".to_string(), 22)));
        assert!(t.contains(&("h1".to_string(), 2222)));
    }

    #[test]
    fn sessions_without_a_host_are_skipped() {
        let s = vec![("x".into(), String::new(), 22u16)];
        assert!(targets(&s).is_empty());
    }

    /// 세 상태가 눈으로 구분돼야 목록에서 읽을 수 있다.
    #[test]
    fn the_three_states_look_different() {
        let m = [Reach::Checking.mark(), Reach::Open.mark(), Reach::Closed.mark()];
        let uniq: std::collections::HashSet<&&str> = m.iter().collect();
        assert_eq!(uniq.len(), 3, "표식이 겹친다: {m:?}");
        assert_ne!(Reach::Open.color(), Reach::Closed.color());
    }

    /// 동시에 여는 수와 기다리는 시간에 상한이 있어야 한다.
    #[test]
    fn the_probe_is_bounded() {
        assert!((1..=64).contains(&PARALLEL));
        assert!(TIMEOUT.as_secs() <= 10);
    }
}
