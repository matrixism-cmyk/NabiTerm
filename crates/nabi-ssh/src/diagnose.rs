//! 연결 실패를 **사람 말로 바꾼다.**
//!
//! 지금까지 실패하면 pane에 `[ssh 오류: <russh 원문>]` 한 줄이 전부였다. 영어 원문 한 줄은
//! 대부분의 사용자에게 아무것도 알려 주지 않는다 — "무설정 한글"이 우리 차별화인데 정작
//! 가장 도움이 필요한 순간에 무너지고 있었다.
//!
//! 그래서 원문을 분류하고, **무슨 일이 있었는지 + 무엇을 해 보면 되는지**를 붙인다.
//! 원문은 지우지 않고 아래에 남긴다(문제를 남에게 물어볼 때 그게 필요하다).
//!
//! 분류는 **순수 함수**다. 문자열 하나를 받아 원인을 돌려주므로 서버 없이 시험할 수 있고,
//! 실제로 그래야 한다 — 실패 경로는 실서버로 재현하기가 가장 어렵다.

use nabi_proto::SshAuth;

/// 인증 **방식만** — 비밀번호나 키 경로는 담지 않는다.
///
/// 진단에 필요한 것은 "무슨 방법을 썼는가"뿐이다. `SshAuth`를 그대로 들고 다니면 실패
/// 경로에서 비밀번호 사본이 하나 더 생긴다. 필요 없는 비밀은 만들지 않는 편이 맞다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthKind {
    Password,
    KeyFile,
    Agent,
    None,
}

impl From<&SshAuth> for AuthKind {
    fn from(a: &SshAuth) -> Self {
        match a {
            SshAuth::Password(_) => AuthKind::Password,
            SshAuth::KeyFile { .. } => AuthKind::KeyFile,
            SshAuth::Agent => AuthKind::Agent,
            SshAuth::None => AuthKind::None,
        }
    }
}

/// 실패의 갈래. 화면 문구와 실마리가 여기서 갈린다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cause {
    /// 이름을 주소로 바꾸지 못했다.
    DnsFailure,
    /// 그 주소와 포트에서 아무도 받지 않았다.
    Refused,
    /// 응답이 없어 기다리다 끊었다.
    Timeout,
    /// 사용자·비밀번호·키를 서버가 받아 주지 않았다.
    AuthFailed,
    /// 개인키 파일 자체의 문제(암호 틀림·형식 깨짐).
    KeyFile,
    /// 호스트키를 신뢰하지 않아 우리가 끊었다.
    HostKey,
    /// 공통 암호 알고리즘이 없다(아주 옛 서버 또는 아주 엄격한 서버).
    Algorithm,
    /// 붙은 뒤에 상대가 끊었다.
    Disconnected,
    /// 갈래를 못 정했다 — 원문을 그대로 보여 준다.
    Unknown,
}

/// 화면에 낼 것: 무슨 일인지 한 줄 + 해 볼 것 몇 줄. 전부 i18n 키다.
#[derive(Clone, Debug)]
pub struct Diagnosis {
    pub cause: Cause,
    pub headline: &'static str,
    pub hints: Vec<&'static str>,
}

/// 오류 원문에서 갈래를 고른다.
///
/// 원문은 겹쳐서 오는 일이 잦다(IO error: Connection refused (os error 10061)). 그래서
/// 앞부분만 보지 않고 **어디에 있든 찾는다.** 윈도우 오류 번호도 같이 본다 — 메시지는
/// 시스템 언어로 번역돼 오지만 번호는 안 바뀐다.
pub fn classify(raw: &str) -> Cause {
    let m = raw.to_ascii_lowercase();
    let has = |n: &str| m.contains(n);
    // 순서가 중요하다: 좁은 것부터 본다. 암호 틀림이 인증 거절보다 먼저다.
    if has("passphrase") || has("key is corrupt") || has("could not read key") || has("keyiscorrupt") {
        return Cause::KeyFile;
    }
    if has("not authenticated") || has("no more auth") || has("permission denied") || has("authentication") {
        return Cause::AuthFailed;
    }
    // russh는 "Unknown server key"라고 말한다 — "unknown key"만 찾으면 못 잡는다(시험이 잡았다).
    if has("unknown key") || has("server key") || has("host key") || has("did not match") || has("rejected by") {
        return Cause::HostKey;
    }
    if has("no common") || has("kex") || has("algorithm") || has("cipher") {
        return Cause::Algorithm;
    }
    // 11001/11004 = 이름 풀이 실패.
    if has("os error 11001") || has("os error 11004") || has("no such host") || has("name or service") {
        return Cause::DnsFailure;
    }
    // 10061 = 거부, 10060 = 시간 초과, 10054 = 상대가 끊음.
    if has("os error 10061") || has("refused") {
        return Cause::Refused;
    }
    if has("os error 10060") || has("timeout") || has("timed out") {
        return Cause::Timeout;
    }
    if has("unexpected eof") || has("early eof") || has("disconnect") || has("reset by peer") || has("os error 10054") {
        return Cause::Disconnected;
    }
    Cause::Unknown
}

/// 갈래와 인증 방식에서 화면 문구를 만든다.
///
/// 인증 실패의 실마리는 **어떤 방법을 썼는지에 따라 완전히 다르다.** 비밀번호를 쓴 사람에게
/// 키 권한을 확인하라고 하면 오히려 헤매게 된다.
pub fn diagnose(raw: &str, auth: AuthKind) -> Diagnosis {
    let cause = classify(raw);
    let (headline, hints): (&str, Vec<&str>) = match cause {
        Cause::DnsFailure => ("ssh.diag.dns", vec!["ssh.diag.dns.h1", "ssh.diag.dns.h2"]),
        Cause::Refused => ("ssh.diag.refused", vec!["ssh.diag.refused.h1", "ssh.diag.refused.h2", "ssh.diag.refused.h3"]),
        Cause::Timeout => ("ssh.diag.timeout", vec!["ssh.diag.timeout.h1", "ssh.diag.timeout.h2"]),
        Cause::AuthFailed => ("ssh.diag.auth", auth_hints(auth)),
        Cause::KeyFile => ("ssh.diag.keyfile", vec!["ssh.diag.keyfile.h1", "ssh.diag.keyfile.h2"]),
        Cause::HostKey => ("ssh.diag.hostkey", vec!["ssh.diag.hostkey.h1", "ssh.diag.hostkey.h2"]),
        Cause::Algorithm => ("ssh.diag.algo", vec!["ssh.diag.algo.h1", "ssh.diag.algo.h2"]),
        Cause::Disconnected => ("ssh.diag.disconnected", vec!["ssh.diag.disconnected.h1"]),
        Cause::Unknown => ("ssh.diag.unknown", vec!["ssh.diag.unknown.h1"]),
    };
    Diagnosis { cause, headline, hints }
}

/// 인증 실패 실마리 — 쓴 방법에 맞는 것만 준다.
fn auth_hints(auth: AuthKind) -> Vec<&'static str> {
    match auth {
        AuthKind::Password => vec!["ssh.diag.auth.pw1", "ssh.diag.auth.pw2", "ssh.diag.auth.common"],
        AuthKind::KeyFile => vec!["ssh.diag.auth.key1", "ssh.diag.auth.key2", "ssh.diag.auth.common"],
        AuthKind::Agent => vec!["ssh.diag.auth.ag1", "ssh.diag.auth.ag2", "ssh.diag.auth.common"],
        AuthKind::None => vec!["ssh.diag.auth.none"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_connection_is_recognised_through_its_wrapper() {
        // 원문은 이렇게 겹쳐서 온다 — 앞부분만 보면 IO error밖에 안 보인다.
        assert_eq!(classify("IO error: Connection refused (os error 10061)"), Cause::Refused);
        assert_eq!(classify("Connection refused"), Cause::Refused);
    }

    /// **번호로도 잡아야 한다** — 메시지는 시스템 언어로 번역돼 오지만 번호는 안 바뀐다.
    #[test]
    fn windows_error_numbers_are_understood_even_when_translated() {
        assert_eq!(classify("연결이 거부되었습니다 (os error 10061)"), Cause::Refused);
        assert_eq!(classify("알 수 없는 호스트입니다 (os error 11001)"), Cause::DnsFailure);
        assert_eq!(classify("시간이 초과되었습니다 (os error 10060)"), Cause::Timeout);
    }

    #[test]
    fn the_usual_failures_each_land_in_their_own_bucket() {
        assert_eq!(classify("Connection timeout"), Cause::Timeout);
        assert_eq!(classify("No such host is known."), Cause::DnsFailure);
        assert_eq!(classify("Not authenticated"), Cause::AuthFailed);
        assert_eq!(classify("Unknown server key"), Cause::HostKey);
        assert_eq!(classify("No common key exchange algorithm"), Cause::Algorithm);
        assert_eq!(classify("Unexpected eof"), Cause::Disconnected);
    }

    /// 좁은 갈래가 넓은 갈래보다 먼저여야 한다 — 안 그러면 전부 한 곳으로 쏠린다.
    #[test]
    fn a_bad_passphrase_is_a_key_problem_not_an_auth_rejection() {
        let m = "Could not read key: wrong passphrase";
        assert_eq!(classify(m), Cause::KeyFile, "인증 거절로 보면 사용자가 서버를 의심하게 된다");
    }

    #[test]
    fn an_unrecognised_error_says_so_instead_of_guessing() {
        assert_eq!(classify("something we have never seen"), Cause::Unknown);
        assert_eq!(classify(""), Cause::Unknown);
    }

    /// **실마리는 쓴 방법에 맞아야 한다.** 비밀번호를 쓴 사람에게 키 권한을 말하면 헤맨다.
    #[test]
    fn the_hints_follow_the_authentication_method() {
        let pw = diagnose("Not authenticated", AuthKind::Password);
        let key = diagnose("Not authenticated", AuthKind::KeyFile);
        let ag = diagnose("Not authenticated", AuthKind::Agent);
        assert_eq!(pw.cause, Cause::AuthFailed);
        assert!(pw.hints.iter().any(|h| h.contains("pw")));
        assert!(key.hints.iter().any(|h| h.contains("key")));
        assert!(ag.hints.iter().any(|h| h.contains("ag")));
        assert_ne!(pw.hints, key.hints, "방법이 다른데 같은 실마리를 주고 있다");
    }

    /// 모든 갈래가 문구와 실마리를 갖춰야 한다 — 하나라도 비면 화면이 텅 빈다.
    #[test]
    fn every_cause_has_something_to_say() {
        for raw in [
            "no such host", "refused", "timeout", "not authenticated",
            "wrong passphrase", "unknown key", "no common algorithm", "unexpected eof", "???",
        ] {
            let d = diagnose(raw, AuthKind::Agent);
            assert!(!d.headline.is_empty(), "{raw}: 제목이 없다");
            assert!(!d.hints.is_empty(), "{raw}: 실마리가 없다");
            assert!(d.headline.starts_with("ssh.diag."), "{raw}: i18n 키가 아니다");
        }
    }
}

/// 진단을 pane에 그대로 찍을 여러 줄 글로 만든다.
///
/// 터미널 화면이므로 CRLF로 끊는다. 원문은 **지우지 않고** 맨 아래에 남긴다 — 남에게
/// 물어볼 때 필요한 것은 우리가 번역한 문장이 아니라 그 원문이다.
pub fn render(raw: &str, auth: AuthKind) -> String {
    let d = diagnose(raw, auth);
    let t = |k: &str| nabi_i18n::trc(k).to_string();
    let mut out = String::new();
    out.push_str("\r\n");
    out.push_str(&format!("[{}] {}\r\n", t("ssh.diag.title"), t(d.headline)));
    out.push_str(&format!("  {}:\r\n", t("ssh.diag.try")));
    for h in &d.hints {
        out.push_str(&format!("   - {}\r\n", t(h)));
    }
    out.push_str(&format!("  {}: {raw}\r\n", t("ssh.diag.raw")));
    out
}

#[cfg(test)]
mod render_tests {
    use super::*;

    /// **원문을 잃으면 안 된다.** 우리 번역은 실마리고, 남에게 물을 때 쓰이는 건 원문이다.
    #[test]
    fn the_original_message_is_always_kept() {
        let raw = "IO error: Connection refused (os error 10061)";
        let out = render(raw, AuthKind::Agent);
        assert!(out.contains(raw), "{out}");
    }

    /// 터미널에 찍히므로 줄바꿈은 CRLF여야 한다 — LF만 쓰면 계단처럼 밀린다.
    #[test]
    fn lines_end_with_crlf_for_the_terminal() {
        let out = render("refused", AuthKind::Agent);
        assert!(out.contains("\r\n"));
        assert!(!out.replace("\r\n", "").contains('\n'), "맨 LF가 섞였다: {out:?}");
    }

    /// 실마리가 한 줄도 안 나오면 진단이 아니라 그냥 오류 메시지다.
    #[test]
    fn at_least_one_hint_is_shown() {
        let out = render("Not authenticated", AuthKind::Password);
        assert!(out.matches(" - ").count() >= 2, "{out}");
    }
}

/// 실제로 실패시켜 보고 화면에 무엇이 찍히는지 눈으로 본다.
///
/// 순수 시험은 분류 규칙만 본다. "정말 저 경로를 지나는가"는 붙여 봐야 안다.
///
/// ```text
/// cargo test -p nabi-ssh real_failures -- --ignored --nocapture
/// ```
#[cfg(test)]
mod real_failures {
    use bytes::Bytes;
    use crossbeam_channel::unbounded;
    use nabi_types::{GridSize, PaneId};
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "실제로 붙어 실패시킨다(네트워크 필요)"]
    async fn a_refused_port_explains_itself() {
        // 아무도 듣고 있지 않을 포트 — 거부가 바로 온다.
        let out = attempt("127.0.0.1", 59_999).await;
        eprintln!("{out}");
        assert!(out.contains(&nabi_i18n::trc("ssh.diag.refused").to_string()), "{out}");
        assert!(out.contains(&nabi_i18n::trc("ssh.diag.try").to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "실제로 붙어 실패시킨다(네트워크 필요)"]
    async fn an_unknown_host_explains_itself() {
        let out = attempt("nabi-no-such-host.invalid", 22).await;
        eprintln!("{out}");
        assert!(out.contains(&nabi_i18n::trc("ssh.diag.dns").to_string()), "{out}");
    }

    async fn attempt(host: &str, port: u16) -> String {
        let (tx, rx) = unbounded::<(PaneId, Bytes)>();
        let params = crate::SshParams {
            host: host.into(),
            port,
            user: "nobody".into(),
            auth: nabi_proto::SshAuth::Agent,
            jump: None,
            agent_forward: false,
        };
        let kh = std::env::temp_dir().join(format!("nabi-diag-{}", std::process::id()));
        let _ = std::fs::remove_file(&kh);
        let _ch = crate::connect(
            &tokio::runtime::Handle::current(),
            PaneId::new(1),
            params,
            GridSize::new(80, 24),
            tx,
            kh.clone(),
            None,
            Box::new(|_| {}),
            None,
        );
        tokio::time::sleep(Duration::from_secs(6)).await;
        let _ = std::fs::remove_file(&kh);
        let mut s = String::new();
        while let Ok((_, b)) = rx.try_recv() {
            s.push_str(&String::from_utf8_lossy(&b));
        }
        s
    }
}
