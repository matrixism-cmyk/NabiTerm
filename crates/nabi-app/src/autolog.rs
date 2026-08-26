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
        if !self.config.terminal.session_log_auto || self.session_logs.contains_key(&pane) {
            return;
        }
        let dir = self.cfg_dir().join("logs");
        if std::fs::create_dir_all(&dir).is_err() {
            return; // 못 만들면 조용히 넘어간다 — 로그 때문에 접속을 막지 않는다.
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let path: PathBuf = dir.join(log_name(host, &stamp));
        self.start_session_log(pane, &path);
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
