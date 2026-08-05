//! 전송 큐 동작 실행 — 일시정지·순서변경·제거·재시도. UI는 sftpqueue.
//!
//! 진행 중인 항목을 멈추는 것은 "취소 후 대기로 되돌리기"다. 남은 `.filepart` 덕분에
//! 다시 시작하면 이어받으므로, 사용자가 보기엔 일시정지처럼 동작한다.

use crate::app::NabiApp;
use crate::sftpqueue::QueueAct;
use crate::sftpxfer::{Transfer, XferState};
use nabi_proto::Command;

/// 대기·정지 항목을 큐 안에서 한 칸 옮긴다(진행 중·완료 항목 위로는 넘어가지 않는다).
///
/// 이미 서버로 보낸 항목의 순서를 바꾸면 큐 표시와 실제 실행 순서가 어긋나므로,
/// 아직 보내지 않은 구간 안에서만 움직인다.
pub(crate) fn move_waiting(q: &mut [Transfer], xfer: u64, by: i32) {
    let Some(i) = q.iter().position(|t| t.xfer == xfer) else { return };
    if q[i].state.finished() || q[i].running() {
        return;
    }
    let j = match by {
        d if d < 0 => match i.checked_sub(1) {
            Some(j) => j,
            None => return,
        },
        _ => i + 1,
    };
    if j >= q.len() {
        return;
    }
    // 옮겨 갈 자리가 이미 보낸(또는 끝난) 항목이면 멈춘다.
    if q[j].running() || q[j].state.finished() {
        return;
    }
    q.swap(i, j);
}

impl NabiApp {
    /// 큐에서 모은 동작을 실행한다.
    pub(crate) fn apply_queue_act(&mut self, a: QueueAct) {
        let Some(id) = self.sftp.id else { return };
        if a.clear {
            self.sftp.transfers.retain(|t| !t.state.finished());
        }
        if a.cancel_all {
            self.orch.send(Command::SftpCancel { id });
        }
        if let Some((x, by)) = a.move_by {
            move_waiting(&mut self.sftp.transfers, x, by);
        }
        if let Some(x) = a.toggle_pause {
            self.toggle_xfer_pause(id, x);
        }
        if let Some(x) = a.retry {
            if let Some(t) = self.sftp.transfers.iter_mut().find(|t| t.xfer == x) {
                t.state = XferState::Waiting; // 명령은 그대로 — 다운로드는 이어받기로 재개된다.
            }
        }
        if let Some(x) = a.remove {
            self.remove_xfer(id, x);
        }
        self.pump_transfers(id);
    }

    /// 일시정지 ↔ 재개. 진행 중이면 중단하고 정지 상태로 둔다.
    fn toggle_xfer_pause(&mut self, id: nabi_proto::SftpId, xfer: u64) {
        let mut stop = false;
        if let Some(t) = self.sftp.transfers.iter_mut().find(|t| t.xfer == xfer) {
            match t.state {
                XferState::Paused => t.state = XferState::Waiting,
                XferState::Waiting => t.state = XferState::Paused,
                XferState::Running => {
                    t.state = XferState::Paused;
                    stop = true;
                }
                _ => {}
            }
        }
        if stop {
            // 지금 이 연결에서 도는 전송을 멈춘다. 동시 실행이 1이라 곧 이 항목이다.
            self.orch.send(Command::SftpCancel { id });
        }
    }

    /// 항목을 큐에서 뺀다(진행 중이면 먼저 중단).
    fn remove_xfer(&mut self, id: nabi_proto::SftpId, xfer: u64) {
        let running = self
            .sftp
            .transfers
            .iter()
            .any(|t| t.xfer == xfer && t.running());
        if running {
            self.orch.send(Command::SftpCancel { id });
        }
        self.sftp.transfers.retain(|t| t.xfer != xfer);
    }
}
