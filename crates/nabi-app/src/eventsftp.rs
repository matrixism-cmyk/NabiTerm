//! SFTP 이벤트 처리 — events.rs에서 분리(파일 크기 규율).
//!
//! 전송 진행률·완료는 큐 항목 식별자(`xfer`)로 지목한다. 위치("첫 미완료 항목")로 찾으면
//! 다른 파일 작업이 끼어들거나 재시도로 순서가 바뀔 때 엉뚱한 행이 갱신된다.

use crate::app::NabiApp;
use nabi_proto::{Event, SftpId};

impl NabiApp {
    /// SFTP 이벤트면 처리하고 `None`을, 아니면 이벤트를 그대로 `Some`으로 돌려준다
    /// (호출측이 이어서 매칭 — 소유권을 잃지 않게).
    pub(crate) fn handle_sftp_event(&mut self, ev: Event, ctx: &egui::Context) -> Option<Event> {
        match ev {
            Event::SftpConnected { id } => self.on_sftp_connected(id, ctx),
            Event::SftpListing { id, path, mut entries } => {
                let (sort, desc) = (self.browser.sort, self.browser.sort_desc); // 활성/배경 동일 정렬.
                if let Some(p) = self.remote_panel_mut(id) {
                    crate::sftpentries::sort_sftp(&mut entries, sort, desc);
                    p.path = path;
                    p.entries = entries;
                    p.status.clear();
                    ctx.request_repaint();
                }
            }
            Event::SftpError { id, message } => self.set_sftp_status(id, message, ctx),
            Event::SftpSearchResults { id, results } => {
                if let Some(p) = self.remote_panel_mut(id) {
                    p.search_results = results;
                    ctx.request_repaint();
                }
            }
            Event::SftpDirSize { id, path, files, dirs, bytes } => {
                let lang = self.lang;
                if let Some(p) = self.remote_panel_mut(id) {
                    let base = crate::sftppath::remote_basename(&path);
                    // 폴더 속성: 이름 · N개 파일 · M개 폴더 · 총 크기.
                    p.status = format!(
                        "{} {} \u{00b7} {}\u{1f4c4} {}\u{1f4c1} \u{00b7} {}",
                        nabi_i18n::tr(lang, "sftp.dirsize"),
                        base,
                        files,
                        dirs,
                        crate::browserfs::human(bytes)
                    );
                    ctx.request_repaint();
                }
            }
            Event::SftpProgress { id, xfer, bytes } => {
                if let Some(p) = self.remote_panel_mut(id) {
                    if let Some(t) = p.transfers.iter_mut().find(|t| t.xfer == xfer) {
                        t.bytes = bytes;
                    }
                    ctx.request_repaint();
                }
            }
            // 파일 작업(삭제·이름변경·권한 등) 완료 — 전송 큐는 건드리지 않는다.
            Event::SftpOpDone { id, name, ok, message } => {
                let msg = if ok { name } else { message };
                self.set_sftp_status(id, msg, ctx);
            }
            Event::SftpTransferDone { id, xfer, name, ok, message } => {
                self.on_transfer_done(id, xfer, name, ok, message, ctx);
            }
            other => return Some(other), // SFTP 이벤트가 아니면 호출측으로 반환.
        }
        None
    }

    /// 패널 상태줄만 갱신하는 이벤트들의 공통 처리.
    fn set_sftp_status(&mut self, id: SftpId, msg: String, ctx: &egui::Context) {
        if let Some(p) = self.remote_panel_mut(id) {
            p.status = msg;
            ctx.request_repaint();
        }
    }

    /// 연결 완료 — 상태 표시 후 홈 디렉터리 목록을 요청한다.
    fn on_sftp_connected(&mut self, id: SftpId, ctx: &egui::Context) {
        let connected = nabi_i18n::tr(self.lang, "sftp.connected").to_string();
        let found = self
            .remote_panel_mut(id)
            .map(|p| {
                p.status = connected;
                p.path = ".".to_string();
            })
            .is_some();
        if found {
            self.orch.send(nabi_proto::Command::SftpList { id, path: ".".to_string() });
            ctx.request_repaint();
        }
    }

    /// 전송 완료 — 해당 큐 항목만 갱신하고, 성공 시 후속 동작(편집기 적재·목록 갱신)을 잇는다.
    fn on_transfer_done(
        &mut self,
        id: SftpId,
        xfer: u64,
        name: String,
        ok: bool,
        message: String,
        ctx: &egui::Context,
    ) {
        let res = self.remote_panel_mut(id).map(|p| {
            let mut was_upload = false;
            if let Some(t) = p.transfers.iter_mut().find(|t| t.xfer == xfer) {
                was_upload = t.up;
                t.done = true;
                t.ok = ok;
                if !ok {
                    t.err = message.clone(); // 항목별 실패 사유 저장(툴팁).
                }
            }
            p.status = if ok { format!("\u{2193} {name}") } else { message };
            let drained = !p.transfers.iter().any(|t| !t.done); // 큐의 마지막 전송인가.
            (was_upload, p.path.clone(), drained)
        });
        let Some((was_upload, path, drained)) = res else { return };
        self.sftp_xfer_notify(ctx, ok, &name, drained); // H6 완료 토스트+attention.
        if ok {
            self.on_edit_download(&name); // 외부 편집기 오픈 또는 내장 에디터 적재.
            if was_upload {
                self.orch.send(nabi_proto::Command::SftpList { id, path });
            }
        }
        ctx.request_repaint();
    }
}
