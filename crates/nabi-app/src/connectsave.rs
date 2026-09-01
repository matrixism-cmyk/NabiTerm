//! Quick Connect 연결/저장/프리필 로직(다이얼로그 UI는 connect.rs).

use crate::app::NabiApp;
use nabi_proto::{Command, SshParams};

impl NabiApp {
    /// `ssh://[user@]host[:port][/path]`(ssh:// 제거된 나머지)로 Quick Connect를 프리필해 연다.
    pub(crate) fn connect_ssh_url(&mut self, rest: &str) {
        let (user, host, port) = parse_ssh_authority(rest);
        self.quick_connect.host = host;
        self.quick_connect.port = port;
        self.quick_connect.user = user;
        self.quick_connect.password.clear();
        self.quick_connect.key_path.clear();
        self.quick_connect.open = true;
    }

    /// `sftp://…` 링크: ssh와 동일 프리필 + SFTP 패널 열기 플래그(터미널 sftp:// Ctrl+클릭).
    pub(crate) fn connect_sftp_url(&mut self, rest: &str) {
        self.connect_ssh_url(rest);
        self.quick_connect.with_sftp = true;
    }

    /// 새 SSH 연결 추가: Quick Connect를 빈 상태로 열어 이름 붙여 등록(Save)하게 한다.
    pub(crate) fn new_ssh_connection(&mut self) {
        self.quick_connect = crate::connect::QuickConnect {
            open: true,
            ..Default::default()
        };
    }

    /// 저장된 SSH 세션으로 SFTP 패널을 연다(자격 해석은 connect_saved와 동일).
    pub(crate) fn open_sftp_saved(&mut self, s: nabi_session::SavedSession, ftp: bool) {
        let nabi_session::SessionKind::Ssh {
            host,
            port,
            user,
            credential_ref,
            key_path,
            jump,
            // SFTP 패널은 셸이 아니라 파일 전송만 한다 — 에이전트를 내줄 이유가 없다.
            agent_forward: _,
        } = s.kind
        else {
            return;
        };
        // 복원 재연결에 쓸 출처를 먼저 복제해 둔다(아래에서 params로 move되기 때문).
        let org = crate::sftppanel::SftpOrigin { cred_ref: credential_ref.clone(), key_path: key_path.clone(), jump: jump.clone() };
        let params = if let Some(pw) = credential_ref.as_ref().and_then(|k| self.vault_get(k)) {
            SshParams::password(host, port, user.clone(), pw)
        } else if let Some(kp) = key_path {
            SshParams::key_file(host, port, user.clone(), kp, None)
        } else {
            self.prefill_ssh(host, port, user);
            return;
        };
        // ProxyJump(D2): 콤마 구분 멀티홉 지원(같은 인증 가정). SFTP도 점프 경유.
        let j = jump.as_deref().unwrap_or("");
        let params = match crate::sshjump::build_jumps(j, &params.auth, &user) {
            crate::sshjump::Jumps::Chain(chain) => params.with_jump(*chain),
            crate::sshjump::Jumps::None => params,
            // 적었는데 틀렸으면 **붙지 않는다.** 점프만 빼고 붙이면 사용자가 적은 것과
            // 다른 길로 나가는데, 그걸 알 방법이 없다.
            crate::sshjump::Jumps::Invalid(bad) => {
                self.notify_jump_error(&bad);
                return;
            }
        };
        self.start_sftp(params, ftp, org);
    }

    /// 저장된 SSH 세션을 Quick Connect에 불러와 수정(이름/호스트/포트/사용자/키)하게 한다.
    pub(crate) fn edit_session(&mut self, s: &nabi_session::SavedSession) {
        if let nabi_session::SessionKind::Ssh {
            host,
            port,
            user,
            key_path,
            credential_ref,
            jump,
            agent_forward,
        } = &s.kind
        {
            // 볼트가 풀려 있으면 저장된 비밀번호를 미리 채운다(★로 가려져도 그대로 저장/연결되게).
            let saved_pw = credential_ref.as_ref().and_then(|k| self.vault_get(k)).unwrap_or_default();
            let qc = &mut self.quick_connect;
            qc.agent_forward = *agent_forward;
            qc.name = s.name.clone();
            qc.folder = s.folder.clone().unwrap_or_default();
            qc.host = host.clone();
            qc.port = port.to_string();
            qc.user = user.clone();
            qc.password = saved_pw;
            qc.key_path = key_path.clone().unwrap_or_default();
            qc.jump = jump.clone().unwrap_or_default(); // 점프 호스트 프리필(B4).
            qc.on_connect = s.on_connect.clone().unwrap_or_default();
            qc.save_password = credential_ref.is_some();
            qc.is_ftp = s.is_ftp;
            qc.with_sftp = s.open_sftp; // "SFTP 함께 열기" 체크 상태 프리필.
            qc.open = true;
        }
    }

    /// Quick Connect를 연다(비어 있으면 최근 호스트/사용자로 프리필).
    pub(crate) fn open_quick_connect(&mut self) {
        if self.quick_connect.host.is_empty() {
            self.quick_connect.host = self.config.terminal.last_host.clone();
            self.quick_connect.user = self.config.terminal.last_user.clone();
            self.quick_connect.port = self.config.terminal.last_port.clone();
        }
        self.quick_connect.open = true;
    }

    /// 현재 Quick Connect 입력을 저장 세션으로 추가/교체하고 영속화한다(비밀 제외).
    pub(crate) fn save_current_session(&mut self) {
        let (custom, folder, host, port, user, pw, key, on_connect, save_pw, is_ftp, with_sftp, jump, fwd) = {
            let qc = &self.quick_connect;
            (
                qc.name.trim().to_string(),
                qc.folder.trim().to_string(),
                qc.host.trim().to_string(),
                qc.port.trim().parse().unwrap_or(22u16),
                qc.user.trim().to_string(),
                qc.password.clone(),
                qc.key_path.trim().to_string(),
                qc.on_connect.trim().to_string(),
                qc.save_password,
                qc.is_ftp,
                qc.with_sftp,
                qc.jump.trim().to_string(),
                qc.agent_forward,
            )
        };
        if host.is_empty() {
            return;
        }
        let cred_key = format!("ssh:{user}@{host}:{port}");
        let wanted_pw = save_pw && !pw.is_empty();
        let credential_ref = if wanted_pw && self.vault_store(&cred_key, &pw) {
            Some(cred_key)
        } else {
            None
        };
        let need_vault = wanted_pw && credential_ref.is_none(); // 볼트 잠금→저장 실패.
        let name = if custom.is_empty() {
            format!("{user}@{host}")
        } else {
            custom
        };
        self.sessions.remove(&name);
        self.sessions.add(nabi_session::SavedSession {
            name: name.clone(),
            folder: (!folder.is_empty()).then_some(folder),
            kind: nabi_session::SessionKind::Ssh {
                host,
                port,
                user,
                credential_ref,
                key_path: (!key.is_empty()).then_some(key),
                jump: (!jump.is_empty()).then_some(jump),
                agent_forward: fwd,
            },
            on_connect: (!on_connect.is_empty()).then_some(on_connect),
            cwd: None,
            is_ftp,
            open_sftp: with_sftp,
            tag: Default::default(),
        });
        self.save_sessions();
        self.vault_unlock_open |= need_vault; // 비번 저장 실패(볼트 잠금)면 볼트 열기.
        let msg = if need_vault {
            nabi_i18n::tr(self.lang, "vault.needunlock").to_string()
        } else {
            format!("{} {name}", nabi_i18n::tr(self.lang, "qc.save"))
        };
        self.notify = Some((msg, std::time::Instant::now()));
    }

    /// host 칸에 "user@host:port"를 붙여넣었으면 각 필드로 분리(연결 직전 호출).
    pub(crate) fn normalize_qc(&mut self) {
        if let Some((u, h, p)) = crate::recent::split_target(&self.quick_connect.host) {
            self.quick_connect.user = u;
            self.quick_connect.host = h;
            if let Some(port) = p {
                self.quick_connect.port = port;
            }
        }
    }

    pub(crate) fn do_quick_connect(&mut self) {
        self.normalize_qc();
        let (host, port, user, pw, key, oncmd, fwd) = {
            let qc = &self.quick_connect;
            (
                qc.host.trim().to_string(),
                qc.port.trim().parse().unwrap_or(22),
                qc.user.trim().to_string(),
                qc.password.clone(),
                qc.key_path.trim().to_string(),
                qc.on_connect.trim().to_string(),
                qc.agent_forward,
            )
        };
        let oncmd = (!oncmd.is_empty()).then_some(oncmd);
        self.config.terminal.last_host = host.clone();
        self.config.terminal.last_user = user.clone();
        self.config.terminal.last_port = port.to_string();
        self.save_config();
        let key_origin = (!key.is_empty()).then(|| key.clone());
        let params = crate::sshauth::params_for(host.clone(), port, user.clone(), pw, &key, false);
        // ProxyJump(B4): 콤마 구분 멀티홉 지원, 같은 인증으로 경유(베스천 동일 자격 가정).
        let jump_str = self.quick_connect.jump.trim().to_string();
        let params = match crate::sshjump::build_jumps(&jump_str, &params.auth, &user) {
            crate::sshjump::Jumps::Chain(chain) => params.with_jump(*chain),
            crate::sshjump::Jumps::None => params,
            crate::sshjump::Jumps::Invalid(bad) => {
                self.notify_jump_error(&bad);
                return;
            }
        };
        let origin = nabi_session::SessionKind::Ssh {
            host,
            port,
            user,
            credential_ref: None,
            key_path: key_origin,
            jump: (!jump_str.is_empty()).then_some(jump_str),
            agent_forward: fwd,
        };
        let seq = self.register_spawn(origin, oncmd);
        self.orch.send(Command::ConnectSsh {
            params,
            size: nabi_types::GridSize::default(),
            scrollback: self.config.terminal.scrollback,
            encoding: self.config.terminal.encoding.clone(),
            stats_secs: self.config.terminal.ssh_stats_secs,
            reply_seq: Some(seq),
        });
    }
}

/// 세션 행 호버 힌트 — 표시 대상 문자열은 SavedSession::target_string(SSOT)로 일원화.
pub(crate) fn conn_hint(s: &nabi_session::SavedSession) -> String {
    let mut h = s.target_string();
    // 식별 정보 보강(호버): 점프 호스트·개인키 경로. 비밀은 표시하지 않는다.
    if let nabi_session::SessionKind::Ssh { key_path, jump, .. } = &s.kind {
        if let Some(j) = jump.as_deref().filter(|j| !j.is_empty()) {
            h.push_str(&format!("\n\u{21aa} {j}"));
        }
        if let Some(k) = key_path.as_deref().filter(|k| !k.is_empty()) {
            h.push_str(&format!("\n\u{1f511} {k}"));
        }
    }
    h
}

/// `[user@]host[:port][/path]` → (user, host, port-문자열). 권한 파싱은 qcparse(SSOT)에 위임.
fn parse_ssh_authority(rest: &str) -> (String, String, String) {
    let authority = rest.split('/').next().unwrap_or(rest);
    match crate::qcparse::parse_target(authority) {
        Some(p) => (p.user.unwrap_or_default(), p.host, p.port.map_or_else(|| "22".to_string(), |n| n.to_string())),
        None => (String::new(), authority.to_string(), "22".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ssh_authority;

    #[test]
    fn parses_ssh_authority_variants() {
        assert_eq!(
            parse_ssh_authority("git@h.io:2222/repo"),
            ("git".into(), "h.io".into(), "2222".into())
        );
        assert_eq!(
            parse_ssh_authority("host"),
            (String::new(), "host".into(), "22".into())
        );
        assert_eq!(
            parse_ssh_authority("u@host"),
            ("u".into(), "host".into(), "22".into())
        );
        assert_eq!(
            parse_ssh_authority("[fe80::1]:22"),
            (String::new(), "fe80::1".into(), "22".into())
        );
        assert_eq!(
            parse_ssh_authority("u@[::1]"),
            ("u".into(), "::1".into(), "22".into())
        );
    }
}
