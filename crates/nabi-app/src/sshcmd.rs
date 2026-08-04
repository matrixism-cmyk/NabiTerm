//! SSH 명령 복사 — 저장 세션/현재 pane의 SSH 출처를 `ssh ...` CLI 명령으로 만들어 클립보드에 복사.
//! 스크립트·공유·문서화용. 비밀번호는 포함하지 않는다(키 경로/점프만). 순수 빌더 + 단위테스트.

use crate::app::NabiApp;
use nabi_i18n::tr;
use nabi_session::SessionKind;
use std::time::Instant;

/// `ssh [-p port] [-i key] [-J jump] [user@]host` 문자열을 만든다(비밀 미포함). 순수.
pub(crate) fn ssh_command(host: &str, port: u16, user: &str, key: Option<&str>, jump: Option<&str>) -> String {
    let mut s = String::from("ssh");
    if port != 22 {
        s.push_str(&format!(" -p {port}"));
    }
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        s.push_str(&format!(" -i \"{k}\""));
    }
    if let Some(j) = jump.filter(|j| !j.is_empty()) {
        s.push_str(&format!(" -J {j}"));
    }
    s.push(' ');
    if user.is_empty() {
        s.push_str(host);
    } else {
        s.push_str(&format!("{user}@{host}"));
    }
    s
}

/// 공개키를 원격 `~/.ssh/authorized_keys`에 추가하는 셸 명령(ssh-copy-id). 작은따옴표는 안전 이스케이프.
pub(crate) fn authorized_keys_cmd(pubkey: &str) -> String {
    let safe = pubkey.trim().replace('\'', "'\\''");
    format!("mkdir -p ~/.ssh && chmod 700 ~/.ssh && printf '%s\\n' '{safe}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && echo 'nabiterm: key installed'")
}

impl NabiApp {
    /// 포커스한 SSH pane(연결된 서버)의 대화형 셸로 공개키 설치 명령을 보낸다(ssh-copy-id 상당, WriteInput 사용).
    pub(crate) fn install_pubkey(&mut self) {
        let Some(p) = self.focused_pane() else { return };
        if !matches!(self.pane_origins.get(&p), Some(SessionKind::Ssh { .. })) {
            self.notify = Some((format!("\u{2715} {}", tr(self.lang, "sshkey.needssh")), Instant::now()));
            return;
        }
        let Some(f) = rfd::FileDialog::new().add_filter("public key", &["pub"]).pick_file() else { return };
        let line = std::fs::read_to_string(&f).ok().and_then(|c| c.lines().next().map(str::to_string));
        let Some(pubkey) = line.filter(|l| !l.trim().is_empty()) else {
            self.notify = Some((format!("\u{2715} {}", tr(self.lang, "sshkey.readfail")), Instant::now()));
            return;
        };
        let mut data = authorized_keys_cmd(&pubkey).into_bytes();
        data.push(b'\r');
        self.orch.send(nabi_proto::Command::WriteInput { pane: p, data: bytes::Bytes::from(data) });
        self.notify = Some((format!("\u{1f511} {}", tr(self.lang, "sshkey.installing")), Instant::now()));
    }

    /// 백그라운드 스레드로 host:port TCP 도달성을 확인(3초)하고 결과를 reach에 기록 후 repaint(연결 테스트).
    pub(crate) fn test_connection(&self, host: String, port: u16, ctx: &egui::Context) {
        let (reach, ctx2, lang) = (self.reach.clone(), ctx.clone(), self.lang);
        std::thread::spawn(move || {
            use std::net::ToSocketAddrs;
            let ok = format!("{host}:{port}").to_socket_addrs().ok().and_then(|mut a| a.next())
                .map(|addr| std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)).is_ok()).unwrap_or(false);
            // 잠금 오염돼도 결과는 남긴다(백그라운드 스레드가 죽어 결과가 사라지지 않게).
            *reach.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(format!("{} {host}:{port}", tr(lang, if ok { "reach.ok" } else { "reach.fail" })));
            ctx2.request_repaint();
        });
    }

    /// 새 ed25519 SSH 키페어를 생성해 사용자가 고른 경로에 개인키·`.pub` 공개키로 저장하고 공개키를 클립보드에 복사한다(PuTTYgen식).
    pub(crate) fn generate_ssh_key(&mut self, ctx: &egui::Context) {
        let comment = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "nabiterm".into());
        let (priv_pem, pub_line, fp) = match nabi_ssh_ext::keygen::generate_ed25519(&comment) {
            Ok(p) => p,
            Err(e) => { self.notify = Some((format!("\u{2715} {e}"), Instant::now())); return; }
        };
        let Some(path) = rfd::FileDialog::new().set_file_name("id_ed25519").save_file() else { return };
        let pubpath = path.with_extension("pub");
        if std::fs::write(&path, &priv_pem).is_ok() && std::fs::write(&pubpath, format!("{pub_line}\n")).is_ok() {
            ctx.copy_text(pub_line); // 공개키를 클립보드로(서버 authorized_keys에 붙여넣기 편의).
            self.quick_connect.key_path = path.display().to_string(); // 빠른 연결에 새 키 프리필(생성→바로 연결).
            self.notify = Some((format!("\u{1f511} {} · {fp}", tr(self.lang, "sshkey.created")), Instant::now())); // 지문도 표시(서버 대조).
        } else {
            self.notify = Some((format!("\u{2715} {}", tr(self.lang, "sshkey.writefail")), Instant::now()));
        }
    }

    /// 포커스 pane이 SSH면 그 접속을 `ssh ...` 명령으로 클립보드에 복사한다.
    pub(crate) fn copy_ssh_command(&mut self, ctx: &egui::Context) {
        let Some(p) = self.focused_pane() else { return };
        if let Some(SessionKind::Ssh { host, port, user, key_path, jump, .. }) = self.pane_origins.get(&p) {
            let cmd = ssh_command(host, *port, user, key_path.as_deref(), jump.as_deref());
            ctx.copy_text(cmd);
            self.notify = Some((tr(self.lang, "ssh.cmdcopied").to_string(), Instant::now()));
        } else {
            self.notify = Some((tr(self.lang, "ssh.notssh").to_string(), Instant::now()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{authorized_keys_cmd, ssh_command};

    #[test]
    fn builds_command() {
        assert_eq!(ssh_command("h", 22, "u", None, None), "ssh u@h");
        assert_eq!(ssh_command("h", 2222, "u", None, None), "ssh -p 2222 u@h");
        assert_eq!(ssh_command("h", 22, "", None, None), "ssh h"); // user 없음.
        assert_eq!(ssh_command("h", 22, "u", Some("k.pem"), Some("b@j")), "ssh -i \"k.pem\" -J b@j u@h");
    }

    #[test]
    fn authkeys_cmd_embeds_key_safely() {
        let c = authorized_keys_cmd("ssh-ed25519 AAAA u@h");
        assert!(c.contains("authorized_keys") && c.contains("ssh-ed25519 AAAA u@h"));
        assert!(c.contains("chmod 600")); // 권한 설정 포함.
        assert!(authorized_keys_cmd("a'b").contains("a'\\''b")); // 작은따옴표 이스케이프.
    }
}
