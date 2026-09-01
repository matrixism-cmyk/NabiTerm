//! 사람이 적은 호스트·사용자 이름이 **설정 파일에 그대로 써도 되는 모양인지** 본다.
//!
//! ## 왜 필요한가
//!
//! 우리는 세션 목록을 OpenSSH `~/.ssh/config` 로 내보낸다(`sshconfig::to_ssh_config`).
//! 그런데 값을 그대로 이어 붙여 쓰고 있었다:
//!
//! ```text
//! Host 내서버
//!     HostName {host}
//!     ProxyJump {jump}
//! ```
//!
//! `host` 에 줄바꿈이 하나 들어 있으면 그 뒤가 **새 지시어 줄**이 된다. 즉
//! `"srv\n    ProxyCommand calc"` 라는 이름 하나로 사용자의 설정 파일에 없던 명령이 생긴다.
//! 이름은 손으로도 적지만 PuTTY·MobaXterm·FileZilla 파일에서 **가져오기도** 하므로,
//! 남이 만든 파일 하나가 그 사람의 ssh 설정을 바꾸는 길이 된다.
//!
//! 2026-09-01 조사에서 OpenSSH 10.3 이 같은 자리를 막은 것을 보고 우리 것을 탐침해 보니
//! `-oProxyCommand=calc`·`host\nProxyCommand calc`·`h;calc` 가 전부 호스트 이름으로
//! 통과하고 있었다.
//!
//! ## 어디서 막는가 — 두 곳 다
//!
//! * **적을 때**(`sshjump`) — 틀린 것은 연결 전에 사람에게 말해 준다.
//! * **쓸 때**(`sshconfig`) — 어떤 길로 들어왔든 파일에는 안전한 것만 나간다.
//!
//! 한 곳만 막으면 다른 길(가져오기·예전에 저장해 둔 세션)로 들어온 것이 그대로 나간다.

/// 설정 파일 한 줄에 값으로 써도 되는가 — 줄을 바꾸거나 주석을 열지 않아야 한다.
///
/// 제어문자(줄바꿈·복귀·탭·NUL)가 없어야 하고, 비어 있지 않아야 한다. 여기서 막는 것은
/// **줄이 갈라지는 것**이다. 호스트로서 말이 되는지는 `valid_host` 가 따로 본다.
pub(crate) fn cfg_safe(v: &str) -> bool {
    !v.is_empty() && !v.chars().any(|c| c.is_control())
}

/// 호스트 이름(또는 IP·IPv6 대괄호 표기)으로 쓸 수 있는 모양인가.
///
/// `-` 로 시작하는 것을 막는 이유: 명령줄에 넘어가면 이름이 아니라 **옵션**으로 읽힌다
/// (`-oProxyCommand=...`). 우리는 러스트 라이브러리로 붙으니 셸을 거치지 않지만,
/// 같은 값이 설정 파일과 화면 안내로도 나가므로 애초에 받지 않는 편이 낫다.
pub(crate) fn valid_host(h: &str) -> bool {
    const OK: &str = ".-_:[]%";
    cfg_safe(h)
        && h.len() <= 253
        && !h.starts_with('-')
        && !h.contains(char::is_whitespace)
        && h.chars().all(|c| c.is_ascii_alphanumeric() || OK.contains(c))
}

/// 로그인 사용자 이름으로 쓸 수 있는 모양인가.
///
/// 계정 이름 규칙은 서버마다 다르니 좁게 잡지 않는다 — **줄을 가르거나 옵션으로 읽히거나
/// `user@host` 를 다시 쪼개게 만드는 글자**만 막는다.
pub(crate) fn valid_user(u: &str) -> bool {
    cfg_safe(u)
        && u.len() <= 64
        && !u.starts_with('-')
        && !u.contains(char::is_whitespace)
        && !u.contains(['@', ',', ':', '/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::{cfg_safe, valid_host, valid_user};

    /// ① 당연히 통과해야 하는 것 — 여기서 막으면 멀쩡한 사람이 못 붙는다.
    #[test]
    fn ordinary_names_pass() {
        for h in ["bastion", "srv-01.example.com", "10.0.0.5", "[fe80::1%eth0]", "a_b.local"] {
            assert!(valid_host(h), "{h:?} 는 통과해야 한다");
        }
        for u in ["root", "kim", "deploy.bot", "u-1", "svc_account"] {
            assert!(valid_user(u), "{u:?} 는 통과해야 한다");
        }
    }

    /// ② 절대 통과하면 안 되는 것 — 탐침으로 실제로 통과하던 것들이다(2026-09-01).
    #[test]
    fn injection_shapes_are_rejected() {
        for h in [
            "-oProxyCommand=calc",         // 이름이 아니라 옵션으로 읽힌다.
            "h -oProxyCommand=calc",       // 뒤에 옵션을 붙였다.
            "host\nProxyCommand calc",     // 설정 파일에 **새 줄**을 만든다.
            "host\r  ProxyCommand calc",   // 복귀문자도 줄을 가른다.
            "h;calc",
            "h`calc`",
            "h$(calc)",
            "h,other",                     // 콤마는 점프 목록 구분자다.
            "",
        ] {
            assert!(!valid_host(h), "{h:?} 는 막아야 한다");
        }
        for u in ["u name", "a@b", "-l", "u\nHost x", ""] {
            assert!(!valid_user(u), "{u:?} 는 막아야 한다");
        }
    }

    /// ③ 설정 파일 경계 — 줄을 가르지만 않으면 통과시킨다(경로·키 파일 이름은 자유롭다).
    #[test]
    fn the_file_boundary_only_blocks_line_breaks() {
        assert!(cfg_safe(r"C:\Users\kim\.ssh\id_ed25519"));
        assert!(cfg_safe("~/키/내 키.pem"), "공백·한글이 든 경로도 값으로는 괜찮다");
        assert!(!cfg_safe("a\nProxyCommand calc"));
        assert!(!cfg_safe("a\0b"));
        assert!(!cfg_safe(""));
    }
}
