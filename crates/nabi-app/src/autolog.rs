//! **세션 로그를 저절로 시작한다** — 켜는 것을 잊어도 기록이 남게.
//!
//! 지금까지 로그는 손으로 켜야 했다(`toggle_session_log`). 그런데 로그가 필요해지는
//! 순간은 대개 **이미 지나간 뒤**다 — 뭔가 잘못됐을 때 "아까 그 출력"을 찾게 된다.
//! 그때 켜 봐야 늦다.
//!
//! ## 어디에 쌓이나
//!
//! 설정 폴더 아래 `logs/`에 `호스트-날짜시각.log`로 쌓인다. 파일 대화상자를 띄우지
//! 않는다 — 저절로 켜지는 것이 매번 창을 띄우면 그건 자동이 아니다.
//!
//! ## 기본은 꺼짐
//!
//! 터미널 출력에는 비밀번호도, 서버 이름도, 남의 데이터도 지나간다. 그것을 **묻지 않고**
//! 디스크에 남기는 것은 프로그램이 마음대로 할 일이 아니다. 켜는 것은 사용자가 정한다.

use crate::app::NabiApp;
use nabi_types::PaneId;
use std::path::PathBuf;

/// 파일 이름에 쓸 수 있게 다듬는다(경로 구분자·금지 문자 제거).
///
/// 호스트 이름에는 `:`(포트)나 사용자가 지은 별칭이 들어온다. 그대로 이어 붙이면 경로가
/// 깨지거나 폴더 밖을 가리킬 수 있다.
pub(crate) fn safe_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
            true => c,
            false => '-',
        })
        .collect();
    let s = s.trim_matches('-').replace("..", "-");
    let cut: String = s.chars().take(40).collect();
    match cut.is_empty() {
        true => "session".to_string(),
        false => cut,
    }
}

/// `호스트-YYYYMMDD-HHMMSS.log` 이름을 짓는다.
pub(crate) fn log_name(host: &str, stamp: &str) -> String {
    format!("{}-{stamp}.log", safe_stem(host))
}

impl NabiApp {
    /// 새 pane이 열렸다 — 설정이 켜져 있으면 로그를 시작한다.
    ///
    /// 이미 로그 중이면 아무것도 하지 않는다(손으로 켠 것을 덮지 않는다).
    pub(crate) fn maybe_autolog(&mut self, pane: PaneId, host: &str) {
        if !self.config.terminal.session_log_auto {
            return;
        }
        self.autolog_now(pane, host);
    }

    /// 설정과 무관하게 **지금부터** 이 pane 을 기록한다.
    ///
    /// 휠 전체 기록이 부른다 — 기록이 없는 pane 에서 과거를 보려 했다면, 적어도
    /// 이 순간부터는 남아야 다음번에는 볼 수 있다.
    pub(crate) fn autolog_now(&mut self, pane: PaneId, host: &str) {
        if self.session_logs.contains_key(&pane) {
            return;
        }
        let dir = self.cfg_dir().join("logs");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            // 접속은 막지 않는다 — 로그가 안 된다고 일을 못 하게 할 이유는 없다.
            //
            // **다만 말은 한다**(배치 AF). 세션 로그는 대개 감사·기록 때문에 켠다. 그런데
            // 켜 놓고 안 남으면 사용자는 남고 있다고 믿고, 나중에 필요할 때 없다는 것을
            // 알게 된다. 그때는 그 세션이 이미 지나갔다.
            //
            // 한 번만 알린다. pane 을 열 때마다 뜨면 곧 읽지 않게 되고, 그러면 없느니만 못하다.
            if !self.autolog_fail_noticed {
                self.autolog_fail_noticed = true;
                self.notify = Some((
                    format!("{} {e}", nabi_i18n::tr(self.lang, "log.dirfailed")),
                    std::time::Instant::now(),
                ));
            }
            return;
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let path: PathBuf = dir.join(log_name(host, &stamp));
        self.start_session_log(pane, &path);
    }
}

impl NabiApp {
    /// 설정이 켜져 있으면 **아직 기록하지 않는 pane 전부**에 기록을 시작한다(배치 AQ).
    ///
    /// ## 왜 이게 필요한가
    ///
    /// 지금까지는 pane 이 **새로 열릴 때만** 기록을 시작했다. 그것도 앱이 직접 연 pane 만이었고,
    /// 제어 평면(`nabi cli spawn`)으로 연 것은 아예 그 길을 타지 않았다.
    ///
    /// 그래서 설정을 켜도 **이미 열려 있던 pane 은 아무 일도 일어나지 않았다.** 켜 두면 남는
    /// 줄 알고 한참 쓴 뒤에야 아무것도 없다는 것을 알게 된다(2026-08-29에 실제로 그랬다).
    ///
    /// 설정 이름은 "모든 세션 기록"이다. 이름대로 하려면 **모든** pane 을 봐야 한다.
    pub(crate) fn sweep_autolog(&mut self) {
        if !self.config.terminal.session_log_auto {
            return;
        }
        let panes: Vec<PaneId> = match self.orch.panes.read() {
            Ok(m) => m.keys().copied().collect(),
            Err(_) => return,
        };
        for pane in panes {
            // 사용자가 이 pane 의 기록을 직접 껐으면 다시 켜지 않는다.
            //
            // 이게 없으면 상태바 REC 를 눌러 꺼도 **다음 프레임에 되살아난다** — 여기가
            // 매 프레임 "기록 안 하는 pane 전부"를 훑기 때문이다. 눌러도 안 꺼지는 것처럼
            // 보였다(사용자 보고 2026-09-05).
            if self.rec_off.contains(&pane) {
                continue;
            }
            if self.session_logs.contains_key(&pane) {
                continue;
            }
            // 출처를 알면 그 이름으로, 모르면 local 로 짓는다.
            let host = match self.pane_origins.get(&pane) {
                Some(nabi_session::SessionKind::Ssh { host, .. }) => host.clone(),
                _ => "local".to_string(),
            };
            self.maybe_autolog(pane, &host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{log_name, safe_stem};

    #[test]
    fn a_plain_host_is_kept() {
        assert_eq!(safe_stem("web01.example.com"), "web01.example.com");
    }

    /// **이름이 경로가 되면 안 된다** — 호스트 이름은 사용자가 짓는다.
    #[test]
    fn a_host_cannot_become_a_path() {
        for evil in ["../../etc", "a/b", "a\\b", "C:", "..\\.."] {
            let s = safe_stem(evil);
            assert!(!s.contains('/') && !s.contains('\\') && !s.contains(".."), "{evil} -> {s}");
        }
    }

    #[test]
    fn an_empty_host_still_gets_a_name() {
        assert_eq!(safe_stem(""), "session");
        assert_eq!(safe_stem("///"), "session");
    }

    /// 아주 긴 이름은 자른다(윈도우 경로 길이 제한).
    #[test]
    fn a_very_long_host_is_cut() {
        assert!(safe_stem(&"h".repeat(200)).len() <= 40);
    }

    /// 이름에 시각이 들어가야 같은 서버에 두 번 붙어도 덮어쓰지 않는다.
    #[test]
    fn two_sessions_to_the_same_host_do_not_collide() {
        let a = log_name("web", "20260826-101500");
        let b = log_name("web", "20260826-101501");
        assert_ne!(a, b);
        assert!(a.ends_with(".log"));
    }

    /// 포트가 붙은 이름도 파일 이름이 된다.
    #[test]
    fn a_host_with_a_port_is_usable() {
        let n = log_name("example.com:2222", "20260826-101500");
        assert!(!n.contains(':'), "{n}");
        assert!(n.starts_with("example.com-2222"), "{n}");
    }
}
