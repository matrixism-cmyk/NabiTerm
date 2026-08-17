//! 제어평면 SFTP(S6-55) — `nabi cli sftp-list/get/put`를 현재 열린 SFTP 연결로 실행.
//!
//! 요청은 seq 상관으로 오고, 완료는 `Event::SftpCtlDone{seq,…}`로 EventHub에 회신한다
//! (dispatch가 그 이벤트를 기다린다 — LayoutExport와 같은 패턴). 전송은 일반 전송 큐를
//! 그대로 타므로 UI 큐·전송 히스토리에도 똑같이 보인다(별도 은닉 경로 없음).

use crate::app::NabiApp;
use nabi_proto::{Command, SftpCtlOp};
use std::collections::HashMap;

/// 진행 중인 제어평면 SFTP 요청의 상관 상태.
#[derive(Default)]
pub struct CtlSftp {
    /// 목록 대기: (원격 경로, 요청 seq) — 같은 경로 동시 요청도 각자 회신받게 Vec.
    pub list: Vec<(String, u64)>,
    /// 전송 대기: 큐 xfer id → 요청 seq. TransferDone(xfer)이 오면 회신.
    pub xfers: HashMap<u64, u64>,
}

impl NabiApp {
    /// AppCtl::SftpCtl 진입점 — 열린 연결이 없으면 즉시 실패 회신.
    pub(crate) fn on_sftp_ctl(&mut self, seq: u64, op: SftpCtlOp) {
        let Some(id) = self.sftp.id else {
            self.sftp_ctl_reply(seq, false, "열린 SFTP 연결이 없습니다 — 먼저 SFTP 탭을 연결하세요".into());
            return;
        };
        match op {
            SftpCtlOp::List { path } => {
                // 같은 경로를 UI가 동시에 요청하는 드문 경우, 제어 회신을 우선한다.
                self.ctl_sftp.list.push((path.clone(), seq));
                self.orch.send(Command::SftpList { id, path });
            }
            SftpCtlOp::Get { remote, local } => {
                let name = crate::sftppath::remote_basename(&remote).to_string();
                let x = self.push_xfer(name, false, 0, |xfer| Command::SftpDownload {
                    id, xfer, remote, local, resume: 0,
                });
                self.ctl_sftp.xfers.insert(x, seq);
            }
            SftpCtlOp::Put { local, remote } => {
                let meta = std::fs::metadata(&local);
                let Ok(meta) = meta else {
                    self.sftp_ctl_reply(seq, false, format!("로컬 파일 없음: {local}"));
                    return;
                };
                let name = crate::sftppath::remote_basename(&remote).to_string();
                let x = self.push_xfer(name, true, meta.len(), |xfer| Command::SftpUpload { id, xfer, local, remote });
                self.ctl_sftp.xfers.insert(x, seq);
            }
        }
    }

    /// SftpListing 이벤트가 제어 요청의 회신인가 — 맞으면 JSON으로 회신하고 true(UI 갱신 생략).
    pub(crate) fn sftp_ctl_take_listing(&mut self, path: &str, entries: &[nabi_proto::SftpEntry]) -> bool {
        let (hit, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ctl_sftp.list).into_iter().partition(|(p, _)| p == path);
        self.ctl_sftp.list = rest;
        if hit.is_empty() {
            return false;
        }
        let data = serde_json::to_string(entries).unwrap_or_else(|_| "[]".into());
        for (_, seq) in hit {
            self.sftp_ctl_reply(seq, true, data.clone());
        }
        true
    }

    /// TransferDone이 제어 요청의 전송인가 — 맞으면 결과를 회신한다(히스토리 기록과 병행).
    pub(crate) fn sftp_ctl_take_xfer(&mut self, xfer: u64, ok: bool, message: &str, name: &str) {
        if let Some(seq) = self.ctl_sftp.xfers.remove(&xfer) {
            let data = if ok { format!("완료: {name}") } else { message.to_string() };
            self.sftp_ctl_reply(seq, ok, data);
        }
    }

    /// SFTP 오류 발생 시: 대기 중인 제어 목록 요청을 실패 회신하고, 동기화 스캔 대기도 푼다.
    /// (오류 이벤트에는 경로가 없어 개별 상관이 불가 — 짧은 수명의 대기라 일괄 해제가 안전.)
    pub(crate) fn sftp_ctl_fail_pending(&mut self, message: &str) {
        for (_, seq) in std::mem::take(&mut self.ctl_sftp.list) {
            self.sftp_ctl_reply(seq, false, message.to_string());
        }
        // (list는 Vec — take로 비웠으니 신규 요청은 이후 자유롭게 쌓인다.)
        if let Some(dlg) = &mut self.sync_dlg {
            dlg.pending = None;
        }
    }

    fn sftp_ctl_reply(&self, seq: u64, ok: bool, data: String) {
        self.control_events.publish(&nabi_proto::Event::SftpCtlDone { seq, ok, data });
    }
}
