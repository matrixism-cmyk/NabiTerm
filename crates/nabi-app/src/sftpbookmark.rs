//! SFTP 원격 경로 북마크 처리(추가/이동/삭제) — sftpact에서 분리(라인 한도).

use crate::app::NabiApp;
use nabi_proto::Command;

impl NabiApp {
    /// 북마크 추가(현재 경로)/이동(go)/삭제(del)를 처리하고 config에 저장한다.
    pub(crate) fn handle_sftp_bookmarks(&mut self, add: bool, go: Option<String>, del: Option<String>) {
        if add {
            let p = self.sftp.path.clone();
            if !p.is_empty() && !self.config.terminal.sftp_bookmarks.contains(&p) {
                self.config.terminal.sftp_bookmarks.push(p);
                let _ = nabi_config::save(&self.config_path, &self.config);
            }
        }
        if let Some(p) = go {
            if let Some(id) = self.sftp.id {
                self.orch.send(Command::SftpList { id, path: p });
            }
        }
        if let Some(p) = del {
            self.config.terminal.sftp_bookmarks.retain(|x| x != &p);
            let _ = nabi_config::save(&self.config_path, &self.config);
        }
    }
}
