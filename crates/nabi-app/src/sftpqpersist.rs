//! 전송 큐 세션 간 영속화(`workspace.squeue`) — 대기·일시정지 항목을 재시작 후 되살린다.
//!
//! 그동안 큐는 메모리에만 있어서, 큰 폴더를 올리다 프로그램을 닫으면 **남은 목록이 통째로
//! 사라졌다**(어디까지 했는지도 알 수 없었다). 진행 중이던 항목은 저장하지 않는다 —
//! 다시 큐에 넣으면 다운로드는 `.filepart`에서, 업로드는 임시 원격 파일에서 이어받는다.
//!
//! 연결은 재시작하면 새 `SftpId`를 받으므로 **명령을 그대로 저장하지 않고** 경로만 저장했다가
//! 붙은 뒤 새 id로 다시 만든다. 어느 탭의 큐인지는 host/user/port로 맞춘다.

use crate::app::NabiApp;
use crate::sftpxfer::XferState;
use nabi_proto::Command;

/// 큐 항목 하나(비밀 없음 — 경로와 크기뿐).
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct QItem {
    pub name: String,
    pub up: bool,
    pub size: u64,
    pub local: String,
    pub remote: String,
    pub is_dir: bool,
}

/// 한 원격 연결의 대기 큐.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct QSave {
    pub host: String,
    pub user: String,
    pub port: String,
    pub items: Vec<QItem>,
}

/// 전송 명령에서 경로·종류를 뽑는다(저장 가능한 것만). 그 외 명령은 큐에 남기지 않는다.
fn item_of(name: &str, up: bool, size: u64, cmd: &Command) -> Option<QItem> {
    let (local, remote, is_dir) = match cmd {
        Command::SftpDownload { remote, local, .. } => (local.clone(), remote.clone(), false),
        Command::SftpDownloadDir { remote, local, .. } => (local.clone(), remote.clone(), true),
        Command::SftpUpload { local, remote, .. } => (local.clone(), remote.clone(), false),
        Command::SftpUploadDir { local, remote, .. } => (local.clone(), remote.clone(), true),
        _ => return None,
    };
    Some(QItem { name: name.to_string(), up, size, local, remote, is_dir })
}

/// 저장 항목 → 실행 명령(새 연결 id·새 xfer 번호로).
fn command_of(it: &QItem, id: nabi_proto::SftpId, xfer: u64) -> Command {
    let (local, remote) = (it.local.clone(), it.remote.clone());
    match (it.up, it.is_dir) {
        (true, true) => Command::SftpUploadDir { id, xfer, local, remote },
        (true, false) => Command::SftpUpload { id, xfer, local, remote },
        // 이어받기 오프셋은 0으로 둔다 — 실제 재개 위치는 남아 있는 `.filepart`가 결정한다.
        (false, true) => Command::SftpDownloadDir { id, xfer, remote, local },
        (false, false) => Command::SftpDownload { id, xfer, remote, local, resume: 0 },
    }
}

impl NabiApp {
    /// 열린 원격 탭들의 **끝나지 않은** 큐 항목을 저장한다(정상 종료 시).
    pub(crate) fn save_xfer_queues(&self) {
        let mut saves: Vec<QSave> = Vec::new();
        for panel in std::iter::once(&self.sftp).chain(self.sftp_bg.values()) {
            if !panel.open || panel.conn_host.is_empty() {
                continue;
            }
            let items: Vec<QItem> = panel
                .transfers
                .iter()
                .filter(|t| keeps(t.state)) // 진행 중이던 것도 포함 — 재개가 이어받는다.
                .filter_map(|t| t.cmd.as_ref().and_then(|c| item_of(&t.name, t.up, t.size, c)))
                .collect();
            if !items.is_empty() {
                saves.push(QSave {
                    host: panel.conn_host.clone(),
                    user: panel.conn_user.clone(),
                    port: panel.conn_port.clone(),
                    items,
                });
            }
        }
        let path = self.workspace_path.with_extension("squeue");
        if saves.is_empty() {
            let _ = std::fs::remove_file(path);
        } else if let Ok(s) = ron::to_string(&saves) {
            let _ = std::fs::write(path, s);
        }
    }

    /// 방금 연결된 원격 탭에 저장된 큐를 되돌린다(호스트/사용자/포트가 같은 항목 1건 소비).
    ///
    /// 소비한 항목은 파일에서 지운다 — 같은 서버에 두 번 붙었을 때 큐가 중복 적재되면
    /// 같은 파일을 두 번 올린다.
    pub(crate) fn restore_xfer_queue(&mut self, id: nabi_proto::SftpId) {
        let path = self.workspace_path.with_extension("squeue");
        let Some(txt) = std::fs::read_to_string(&path).ok() else { return };
        let Ok(mut saves) = ron::from_str::<Vec<QSave>>(&txt) else { return };
        let Some(panel) = self.remote_panel_mut(id) else { return };
        let (h, u, p) = (panel.conn_host.clone(), panel.conn_user.clone(), panel.conn_port.clone());
        let Some(i) = saves.iter().position(|s| s.host == h && s.user == u && s.port == p) else {
            return;
        };
        let taken = saves.remove(i);
        // 남은 큐를 도로 기록(다른 탭 몫). 비면 파일을 지운다.
        if saves.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else if let Ok(s) = ron::to_string(&saves) {
            let _ = std::fs::write(&path, s);
        }
        for it in &taken.items {
            let (name, up, size) = (it.name.clone(), it.up, it.size);
            let it2 = QItem { name: name.clone(), up, size, local: it.local.clone(), remote: it.remote.clone(), is_dir: it.is_dir };
            self.push_xfer(name, up, size, move |xfer| command_of(&it2, id, xfer));
        }
    }
}

/// 이 상태의 항목을 저장할 것인가 — 완료·실패는 큐에 남기지 않는다.
fn keeps(state: XferState) -> bool {
    !state.finished()
}

#[cfg(test)]
mod tests {
    use super::{command_of, item_of, keeps, QItem};
    use crate::sftpxfer::XferState;
    use nabi_proto::Command;

    fn q(up: bool, is_dir: bool) -> QItem {
        QItem { name: "f".into(), up, size: 9, local: "C:/a".into(), remote: "/b".into(), is_dir }
    }

    #[test]
    fn finished_items_are_dropped() {
        assert!(keeps(XferState::Waiting) && keeps(XferState::Paused) && keeps(XferState::Running));
        assert!(!keeps(XferState::Done) && !keeps(XferState::Failed));
    }

    #[test]
    fn item_roundtrips_through_command() {
        let id = 7;
        for (up, is_dir) in [(true, true), (true, false), (false, true), (false, false)] {
            let cmd = command_of(&q(up, is_dir), id, 3);
            let back = item_of("f", up, 9, &cmd).expect("전송 명령이어야 한다");
            assert_eq!((back.up, back.is_dir), (up, is_dir));
            assert_eq!((back.local.as_str(), back.remote.as_str()), ("C:/a", "/b"));
        }
    }

    #[test]
    fn non_transfer_commands_are_ignored() {
        assert!(item_of("f", false, 0, &Command::SftpClose { id: 1 }).is_none());
    }

    #[test]
    fn ron_roundtrip() {
        let v = vec![super::QSave { host: "h".into(), user: "u".into(), port: "22".into(), items: vec![q(true, false)] }];
        let s = ron::to_string(&v).unwrap();
        let back: Vec<super::QSave> = ron::from_str(&s).unwrap();
        assert_eq!(back[0].items[0].remote, "/b");
    }
}
