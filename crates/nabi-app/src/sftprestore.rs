//! SFTP/FTP 브라우저 탭의 워크스페이스 저장·복원(`workspace.stabs`).
//!
//! 터미널 탭은 `workspace.toml`(SavedSession)로 복원되지만 원격 브라우저 탭은 출처 모델이
//! 달라 그동안 복원 대상에서 빠져 있었다(사용자 요청 2026-08-19: 로컬·SFTP 둘 다 복원).
//! 비밀번호는 저장하지 않는다 — 볼트 키(`cred_ref`)나 키 파일 경로만 남기고, 둘 다 없으면
//! 복원 시 접속 창을 미리 채워 띄운다(기존 `open_sftp_saved`와 같은 규칙).

use crate::app::NabiApp;
use nabi_types::PaneId;

/// 원격 탭 1개의 저장 항목. RON 튜플이 아니라 이름 있는 구조체 — 필드 추가에 강하다.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct SftpSave {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub cred_ref: Option<String>,
    pub key_path: Option<String>,
    pub jump: Option<String>,
    /// 마지막 원격 경로(비면 서버 기본 디렉터리).
    pub path: String,
    pub is_ftp: bool,
    pub view: u8,
    pub show_hidden: bool,
}

impl NabiApp {
    /// 도크에 열려 있는 원격 탭들을 도크 순서대로 저장한다(정상 종료 시).
    pub(crate) fn save_sftp_tabs(&self) {
        let saves: Vec<SftpSave> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, p)| self.sftp_panel_at(*p))
            .filter(|s| s.open && !s.conn_host.is_empty())
            .map(|s| SftpSave {
                host: s.conn_host.clone(),
                port: s.conn_port.parse().unwrap_or(22),
                user: s.conn_user.clone(),
                cred_ref: s.cred_ref.clone(),
                key_path: s.key_path.clone(),
                jump: s.jump.clone(),
                path: s.path.clone(),
                is_ftp: s.is_ftp,
                view: s.view_mode.to_u8(),
                show_hidden: s.show_hidden,
            })
            .collect();
        let path = self.workspace_path.with_extension("stabs");
        if saves.is_empty() {
            let _ = std::fs::remove_file(path);
        } else if let Ok(s) = ron::to_string(&saves) {
            // 삼킴: 원격 탭 목록이다. 못 남기면 다시 켤 때 안 되살아난다.
            let _ = std::fs::write(path, s);
        }
    }

    /// PaneId로 원격 패널을 찾는다(활성은 self.sftp, 나머지는 sftp_bg).
    pub(crate) fn sftp_panel_at(&self, pane: PaneId) -> Option<&crate::sftppanel::SftpPanel> {
        if Some(pane) == self.sftp_pane {
            return Some(&self.sftp);
        }
        self.sftp_bg.get(&pane)
    }

    /// 저장된 원격 탭들을 다시 연결한다. 만들어진 PaneId를 저장 순서대로 돌려준다
    /// (레이아웃 서수 2000+i 매핑용). 자격증명이 없으면 그 항목은 접속 창 프리필로 끝난다.
    pub(crate) fn restore_sftp_tabs(&mut self) -> Vec<PaneId> {
        let mut out = Vec::new();
        let Some(txt) = std::fs::read_to_string(self.workspace_path.with_extension("stabs")).ok()
        else {
            return out;
        };
        // 원격 탭도 같다. 하나가 깨져도 나머지는 다시 붙는다(브라우저 탭과 같은 길).
        let (saves, _dropped) = crate::ronsalvage::parse_vec::<SftpSave>(&txt);
        for s in saves {
            let before = self.sftp_pane;
            let want_path = s.path.clone();
            let (view, hidden) = (s.view, s.show_hidden);
            self.open_sftp_saved(to_session(s), false);
            // 자격증명이 없으면 open_sftp_saved가 프리필만 하고 탭을 안 만든다 — 그때는 건너뛴다.
            let Some(p) = self.sftp_pane.filter(|p| Some(*p) != before) else { continue };
            self.sftp.view_mode = crate::sftpview::ViewMode::from_u8(view);
            self.sftp.show_hidden = hidden;
            // 접속 완료 이벤트가 이 경로로 목록을 요청한다(비어 있으면 서버 기본).
            self.sftp.restore_path = (!want_path.is_empty()).then_some(want_path);
            out.push(p);
        }
        out
    }
}

/// 저장 항목 → SavedSession(기존 `open_sftp_saved` 경로 재사용, SSOT).
fn to_session(s: SftpSave) -> nabi_session::SavedSession {
    nabi_session::SavedSession {
        name: String::new(), // 워크스페이스 복원은 '마지막 접속' 기록을 남기지 않는다.
        folder: None,
        kind: nabi_session::SessionKind::Ssh {
            host: s.host,
            port: s.port,
            user: s.user,
            credential_ref: s.cred_ref,
            key_path: s.key_path,
            jump: s.jump,
            agent_forward: false,
        },
        on_connect: None,
        cwd: None,
        is_ftp: s.is_ftp,
        open_sftp: false,
        tag: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{to_session, SftpSave};

    fn sample() -> SftpSave {
        SftpSave {
            host: "example.com".into(),
            port: 2222,
            user: "kim".into(),
            cred_ref: Some("vault:1".into()),
            key_path: None,
            jump: Some("bastion".into()),
            path: "/srv/www".into(),
            is_ftp: false,
            view: 2,
            show_hidden: true,
        }
    }

    #[test]
    fn ron_roundtrip_keeps_fields() {
        let v = vec![sample()];
        let s = ron::to_string(&v).unwrap();
        let back: Vec<SftpSave> = ron::from_str(&s).unwrap();
        assert_eq!(back[0].host, "example.com");
        assert_eq!(back[0].port, 2222);
        assert_eq!(back[0].path, "/srv/www");
        assert!(back[0].show_hidden);
        assert_eq!(back[0].cred_ref.as_deref(), Some("vault:1"));
    }

    #[test]
    fn to_session_maps_ssh_fields() {
        let s = to_session(sample());
        assert!(!s.is_ftp);
        assert!(s.name.is_empty()); // 복원은 '마지막 접속' 기록을 남기지 않는다.
        match s.kind {
            nabi_session::SessionKind::Ssh { host, port, user, credential_ref, jump, .. } => {
                assert_eq!((host.as_str(), port, user.as_str()), ("example.com", 2222, "kim"));
                assert_eq!(credential_ref.as_deref(), Some("vault:1"));
                assert_eq!(jump.as_deref(), Some("bastion"));
            }
            _ => panic!("SSH 여야 한다"),
        }
    }

    /// 저장 파일에 없던 필드가 생겨도 과거 파일을 읽을 수 있어야 한다(serde default).
    #[test]
    fn missing_optional_fields_are_none() {
        let s = r#"[(host:"h",port:22,user:"u",cred_ref:None,key_path:None,jump:None,path:"",is_ftp:true,view:0,show_hidden:false)]"#;
        let back: Vec<SftpSave> = ron::from_str(s).unwrap();
        assert!(back[0].is_ftp && back[0].path.is_empty());
    }
}
