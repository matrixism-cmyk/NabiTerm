//! 동기 브라우징(FileZilla식): 켜면 로컬·원격 두 페인이 같은 상대 경로로 함께 이동한다.
//! 토글 시 현재 로컬/원격 경로를 "루트"로 잡고, 이후 한쪽 이동을 상대 경로로 반대편에 반영.

use crate::app::NabiApp;
use nabi_proto::Command;
use std::path::{Path, PathBuf};

/// 원격(POSIX) 경로를 로컬로 매핑: remote_path가 remote_root 하위면 그 상대분을 local_root에 붙인다.
pub(crate) fn synced_local(
    remote_root: &str,
    remote_path: &str,
    local_root: &Path,
) -> Option<PathBuf> {
    let rr = remote_root.trim_end_matches('/');
    let rel = remote_path.strip_prefix(rr)?;
    let mut p = local_root.to_path_buf();
    for seg in rel.split('/').filter(|s| !s.is_empty()) {
        p.push(seg);
    }
    Some(p)
}

/// 로컬 경로를 원격(POSIX)으로 매핑: local_path가 local_root 하위면 그 상대분을 remote_root에 붙인다.
pub(crate) fn synced_remote(
    local_root: &Path,
    local_path: &Path,
    remote_root: &str,
) -> Option<String> {
    let rel = local_path.strip_prefix(local_root).ok()?;
    let mut out = remote_root.trim_end_matches('/').to_string();
    for seg in rel.components() {
        out.push('/');
        out.push_str(&seg.as_os_str().to_string_lossy());
    }
    if out.is_empty() {
        out.push('/');
    }
    Some(out)
}

impl NabiApp {
    /// 동기 브라우징 토글 — 켤 때 현재 로컬/원격 경로를 루트로 캡처한다.
    pub(crate) fn toggle_sync_browse(&mut self) {
        self.sync_browse = !self.sync_browse;
        if self.sync_browse {
            self.sync_local_root = self.browser.path.clone();
            self.sync_remote_root = self.sftp.path.clone();
            self.browser.open = true;
        }
    }

    /// 원격 이동 후 로컬 페인을 같은 상대 경로로 맞춘다(실재 디렉터리만).
    pub(crate) fn sync_after_remote_nav(&mut self, new_remote: &str) {
        if !self.sync_browse {
            return;
        }
        if let Some(local) = synced_local(&self.sync_remote_root, new_remote, &self.sync_local_root) {
            if local.is_dir() {
                self.browser.path = local;
                self.browser.open = true;
            }
        }
    }

    /// 로컬 이동 후 원격 페인을 같은 상대 경로로 맞춘다.
    pub(crate) fn sync_after_local_nav(&mut self, new_local: &Path) {
        if !self.sync_browse {
            return;
        }
        let Some(id) = self.sftp.id else { return };
        if let Some(remote) = synced_remote(&self.sync_local_root, new_local, &self.sync_remote_root)
        {
            self.sftp.path = remote.clone();
            self.orch.send(Command::SftpList { id, path: remote });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{synced_local, synced_remote};
    use std::path::Path;

    #[test]
    fn remote_to_local_relative() {
        let p = synced_local("/home/u", "/home/u/src/app", Path::new("C:/proj")).unwrap();
        assert_eq!(p, Path::new("C:/proj/src/app"));
        // 루트 하위가 아니면 None.
        assert!(synced_local("/home/u", "/etc", Path::new("C:/proj")).is_none());
    }

    #[test]
    fn local_to_remote_relative() {
        let r = synced_remote(Path::new("C:/proj"), Path::new("C:/proj/src/app"), "/home/u");
        assert_eq!(r.unwrap(), "/home/u/src/app");
    }
}
