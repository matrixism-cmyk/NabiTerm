//! SFTP 선택(단일/Ctrl 토글/Shift 범위)·경로 이동 공통 처리 — sftpact에서 분리.

use crate::app::NabiApp;
use nabi_proto::Command;

impl NabiApp {
    /// 원격 경로 이동 공통 처리(선택 해제·검색 종료·목록 요청·동기 브라우징).
    pub(crate) fn remote_nav(&mut self, id: u64, path: String) {
        let path = crate::sftppath::normalize(&path); // `.`/`..`/중복 슬래시 정리.
        self.sftp.path = path.clone();
        self.sftp.selected = None;
        self.sftp.multi.clear();
        self.sftp.search_results.clear();
        self.orch.send(Command::SftpList {
            id,
            path: path.clone(),
        });
        self.sync_after_remote_nav(&path);
    }

    /// 클릭 선택 적용: 일반=단일, Ctrl=토글, Shift=anchor→대상 범위(목록 순서 기준).
    pub(crate) fn sftp_apply_select(&mut self, sel: Option<(String, bool, bool)>) {
        let Some((s, ctrl, shift)) = sel else { return };
        let p = &mut self.sftp;
        if ctrl {
            if !p.multi.remove(&s) {
                p.multi.insert(s.clone());
            }
        } else if shift {
            let names: Vec<&str> = p.entries.iter().map(|e| e.name.as_str()).collect();
            let i1 = p.selected.as_deref().and_then(|x| names.iter().position(|n| *n == x));
            let i2 = names.iter().position(|n| *n == s);
            if let (Some(x), Some(y)) = (i1, i2) {
                p.multi.clear();
                for n in &names[x.min(y)..=x.max(y)] {
                    p.multi.insert((*n).to_string());
                }
            }
        } else {
            p.multi.clear();
            p.multi.insert(s.clone());
        }
        p.selected = Some(s);
    }
}
