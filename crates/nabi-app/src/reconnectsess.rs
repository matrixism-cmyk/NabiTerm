//! **재접속할 때 어느 저장 세션으로 돌아갈지 고른다.**
//!
//! ## 무엇이 문제였나
//!
//! 자동 재접속은 pane의 출처(`SessionKind`)만 들고 세션을 새로 지어 붙였다. 이름이 빈 채로.
//!
//! ```text
//! SavedSession { name: String::new(), on_connect: None, ... }
//! ```
//!
//! 그런데 세션에 걸어 둔 **터널 규칙은 세션 이름을 열쇠로** 설정에 산다(`auto_forwards`).
//! 이름이 비었으니 아무것도 찾지 못하고, **재접속이 성공해도 터널은 돌아오지 않았다.**
//! 접속 후 자동 실행 명령(`on_connect`)도 같이 사라졌다.
//!
//! 자동 재접속의 목적은 "자리를 비운 사이 알아서 돌아와 있는 것"인데, 돌아온 자리에 터널이
//! 없으면 그 터널로 붙어 있던 도구가 조용히 죽는다. 화면에는 재접속 성공만 보이므로
//! 원인을 찾기 어렵다.
//!
//! ## 어떻게 고르나
//!
//! 출처가 같은 저장 세션을 찾는다. 같은 접속 정보로 저장된 세션이 여럿일 수 있다
//! (이름만 다른 사본, 폴더만 다른 것). 그럴 때는 **가장 위험한 표식**을 가진 것을 고른다 —
//! `pane_tag`가 이미 같은 이유로 그렇게 한다. 운영 세션의 터널을 개발 세션 규칙으로 여는
//! 것보다, 개발 세션에 운영 규칙을 다는 편이 눈에 띄고 덜 위험하다.
//!
//! 못 찾으면(저장하지 않고 빠른 연결로 붙은 경우) 예전처럼 이름 없는 세션으로 돌아간다.

use nabi_session::{SavedSession, SessionKind};

/// 출처가 같은 저장 세션을 고른다. 없으면 None.
pub(crate) fn pick<'a>(saved: &'a [SavedSession], kind: &SessionKind) -> Option<&'a SavedSession> {
    saved
        .iter()
        .filter(|s| &s.kind == kind)
        .max_by_key(|s| (s.tag.is_risky(), std::cmp::Reverse(s.name.len())))
}

/// 재접속에 쓸 세션을 만든다 — 저장된 것이 있으면 그대로, 없으면 출처만 든 최소 세션.
///
/// 그대로 쓰는 것이 중요하다. 여기서 필드를 골라 담기 시작하면 세션에 새 필드가 생길 때마다
/// 이 자리를 함께 고쳐야 하고, 잊으면 **재접속에서만 조용히 빠진다**(이번 결함이 그랬다).
pub(crate) fn session_for(saved: &[SavedSession], kind: &SessionKind) -> SavedSession {
    if let Some(s) = pick(saved, kind) {
        return s.clone();
    }
    SavedSession {
        name: String::new(),
        folder: None,
        kind: kind.clone(),
        on_connect: None,
        cwd: None,
        is_ftp: false,
        open_sftp: false,
        tag: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_session::SessionTag;

    fn ssh(host: &str) -> SessionKind {
        SessionKind::Ssh {
            host: host.into(),
            port: 22,
            user: "u".into(),
            credential_ref: None,
            key_path: None,
            jump: None,
            agent_forward: false,
        }
    }

    fn sess(name: &str, host: &str, tag: SessionTag) -> SavedSession {
        SavedSession {
            name: name.into(),
            folder: None,
            kind: ssh(host),
            on_connect: Some("tmux attach".into()),
            cwd: None,
            is_ftp: false,
            open_sftp: false,
            tag,
        }
    }

    /// **핵심** — 재접속이 이름을 되찾아야 터널 규칙을 찾을 수 있다.
    #[test]
    fn the_saved_session_comes_back_with_its_name() {
        let saved = vec![sess("웹서버", "a", SessionTag::None)];
        let got = session_for(&saved, &ssh("a"));
        assert_eq!(got.name, "웹서버", "이름이 없으면 터널 규칙을 찾지 못한다");
    }

    /// 접속 후 자동 실행 명령도 함께 돌아와야 한다 — 예전에는 None으로 지워졌다.
    #[test]
    fn the_after_connect_command_comes_back_too() {
        let saved = vec![sess("웹서버", "a", SessionTag::None)];
        assert_eq!(session_for(&saved, &ssh("a")).on_connect.as_deref(), Some("tmux attach"));
    }

    /// 저장하지 않고 붙은 세션(빠른 연결)은 예전처럼 이름 없이 돌아간다 — 되기는 해야 한다.
    #[test]
    fn an_unsaved_connection_still_reconnects() {
        let got = session_for(&[], &ssh("a"));
        assert!(got.name.is_empty());
        assert_eq!(got.kind, ssh("a"));
    }

    /// 접속 정보가 다르면 남의 세션을 집어오면 안 된다.
    #[test]
    fn a_different_host_is_not_borrowed() {
        let saved = vec![sess("웹서버", "a", SessionTag::None)];
        assert!(pick(&saved, &ssh("b")).is_none());
    }

    /// 같은 접속 정보가 여럿이면 **가장 위험한 표식**을 따른다(안전한 쪽으로 틀린다).
    #[test]
    fn the_riskiest_tag_wins_when_several_match() {
        let saved = vec![
            sess("개발", "a", SessionTag::Dev),
            sess("운영", "a", SessionTag::Prod),
        ];
        let got = session_for(&saved, &ssh("a"));
        assert_eq!(got.tag, SessionTag::Prod, "덜 위험한 쪽을 골랐다");
    }

    /// 표식이 같으면 고르는 값이 흔들리지 않아야 한다 — 재접속마다 다른 세션을 쓰면
    /// 어떤 때는 터널이 열리고 어떤 때는 안 열린다.
    #[test]
    fn the_choice_is_stable_when_tags_tie() {
        let saved = vec![
            sess("짧은", "a", SessionTag::None),
            sess("아주 긴 이름", "a", SessionTag::None),
        ];
        let a = session_for(&saved, &ssh("a")).name;
        let b = session_for(&saved, &ssh("a")).name;
        assert_eq!(a, b);
    }
}
