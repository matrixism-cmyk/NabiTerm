//! 빠른 연결 입력(비밀번호·키 파일)에서 인증 방식을 고른다 — 순수 함수, 테스트 대상.
//!
//! 규칙은 `ssh(1)`을 따른다: 키 파일을 적었으면 키, 비밀번호를 적었으면 비밀번호,
//! **둘 다 비었으면 실행 중인 ssh-agent**. 예전에는 마지막 경우에 빈 비밀번호를 보내
//! 무조건 실패했다 — 에이전트에 키를 올려 두고 쓰는 사람이 접속할 방법이 없었다.

use nabi_proto::SshParams;

/// 어떤 인증으로 붙을지 정한다.
///
/// `ftp`면 에이전트·키가 의미 없으므로 항상 비밀번호(FTP 프로토콜엔 공개키 인증이 없다).
pub(crate) fn params_for(
    host: String,
    port: u16,
    user: String,
    pw: String,
    key: &str,
    ftp: bool,
) -> SshParams {
    if !ftp && !key.is_empty() {
        let pass = (!pw.is_empty()).then_some(pw);
        return SshParams::key_file(host, port, user, key, pass);
    }
    if !ftp && pw.is_empty() {
        return SshParams::agent(host, port, user);
    }
    SshParams::password(host, port, user, pw)
}

#[cfg(test)]
mod tests {
    use super::params_for;
    use nabi_proto::SshAuth;

    fn auth(pw: &str, key: &str, ftp: bool) -> SshAuth {
        params_for("h".into(), 22, "u".into(), pw.into(), key, ftp).auth
    }

    #[test]
    fn key_wins_over_password() {
        let a = auth("pw", "~/.ssh/id_ed25519", false);
        let SshAuth::KeyFile { path, passphrase } = a else { panic!("키 인증이어야 한다") };
        assert_eq!(path, "~/.ssh/id_ed25519");
        assert_eq!(passphrase.as_deref(), Some("pw"), "비밀번호 칸은 키 암호로 쓴다");
    }

    #[test]
    fn password_when_typed() {
        assert!(matches!(auth("pw", "", false), SshAuth::Password(p) if p == "pw"));
    }

    /// 비어 있으면 에이전트 — 빈 비밀번호를 보내 실패하던 자리다.
    #[test]
    fn empty_falls_back_to_agent() {
        assert!(matches!(auth("", "", false), SshAuth::Agent));
    }

    /// FTP는 공개키 인증이 없다 — 키를 적었어도 비밀번호로 간다.
    #[test]
    fn ftp_always_password() {
        assert!(matches!(auth("pw", "~/.ssh/id_rsa", true), SshAuth::Password(_)));
        assert!(matches!(auth("", "", true), SshAuth::Password(p) if p.is_empty()));
    }
}
