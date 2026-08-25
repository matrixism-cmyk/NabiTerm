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
                // 제어평면(sftp-list) 회신이면 브라우저 UI를 건드리지 않는다(S6-55).
                if self.sftp_ctl_take_listing(&path, &entries) {
                    return None;
                }
                let (sort, desc) = (self.browser.sort, self.browser.sort_desc); // 활성/배경 동일 정렬.
                if let Some(p) = self.remote_panel_mut(id) {
                    crate::sftpentries::sort_sftp(&mut entries, sort, desc);
                    p.path = path;
                    p.entries = entries;
                    p.status.clear();
                    ctx.request_repaint();
                }
            }
            Event::SftpError { id, message } => {
                self.sftp_ctl_fail_pending(&message); // 제어 목록·동기화 스캔 대기 해제(S6-55/51).
                self.set_sftp_status(id, message, ctx);
            }
            Event::SftpSearchResults { id, results } => {
                if let Some(p) = self.remote_panel_mut(id) {
                    p.search_results = results;
                    ctx.request_repaint();
                }
            }
            Event::SftpTree { seq, files, .. } => self.on_sync_tree(seq, files),
            Event::SftpPreview { id, path, data, more, err } => {
                if self.sftp.id == Some(id) {
                    self.preview_arrived(path, data, more, err);
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
        // 워크스페이스 복원이면 저장된 경로로 바로 들어간다(없으면 서버 기본 ".").
        let mut listpath = ".".to_string();
        let found = self
            .remote_panel_mut(id)
            .map(|p| {
                p.status = connected;
                if let Some(want) = p.restore_path.take().filter(|s| !s.trim().is_empty()) {
                    listpath = want;
                }
                p.path = listpath.clone();
            })
            .is_some();
        if found {
            self.restore_xfer_queue(id); // 재시작 전 남아 있던 대기 큐를 이 연결로 되살린다.
            self.orch.send(nabi_proto::Command::SftpList { id, path: listpath });
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
        use crate::sftpxfer::XferState;
        let res = self.remote_panel_mut(id).map(|p| {
            let mut was_upload = false;
            let mut rec = None; // 전송 히스토리 기록(S6-60): (up, size, 소요초).
            if let Some(t) = p.transfers.iter_mut().find(|t| t.xfer == xfer) {
                was_upload = t.up;
                // 사용자가 멈춘 항목의 취소 완료는 정지 상태를 유지한다(실패로 표시하지 않는다).
                if t.state != XferState::Paused {
                    t.state = if ok { XferState::Done } else { XferState::Failed };
                    rec = Some((t.up, t.size, t.started.elapsed().as_secs_f64()));
                }
                if !ok {
                    t.err = message.clone(); // 항목별 실패 사유 저장(툴팁).
                }
            }
            p.status = if ok { format!("\u{2193} {name}") } else { message.clone() };
            if ok && was_upload {
                p.dir_stale = true; // 목록 갱신은 큐가 빈 뒤 한 번만(아래 refresh).
            }
            // 큐가 다 비었는가(대기·정지 항목이 남아 있으면 아직 진행 중인 큐다).
            let drained = !p.transfers.iter().any(|t| !t.state.finished());
            // 갱신 판단은 `drained`가 아니라 `settled` — 일시정지 항목 하나가 목록 갱신을
            // 영영 막으면 안 된다(완료 토스트는 여전히 "전부 끝남" 기준이다).
            let quiet = crate::sftpxfer::settled(&p.transfers);
            let refresh = crate::sftpxfer::take_refresh(quiet, &mut p.dir_stale);
            (refresh, p.path.clone(), drained, rec)
        });
        self.sftp_ctl_take_xfer(xfer, ok, &message, &name); // 제어평면 회신은 패널 유무와 무관(닫혀도 CLI가 기다린다).
        let Some((refresh, path, drained, rec)) = res else { return };
        if let Some((up, size, secs)) = rec {
            self.record_xfer(&name, up, ok, size, secs, &message); // 전송 히스토리(S6-60).
        }
        self.sftp_xfer_notify(ctx, ok, &name, drained); // H6 완료 토스트+attention.
        if ok {
            self.on_edit_download(&name); // 외부 편집기 오픈 또는 내장 에디터 적재.
        }
        // 업로드마다 목록을 다시 받으면, 여러 전송이 동시에 도는 동안 파일 목록이 계속
        // 갈아엎힌다(24개를 올리면 24번). 큐가 비었을 때 한 번만 받는다.
        if refresh {
            self.orch.send(nabi_proto::Command::SftpList { id, path });
        }
        self.pump_transfers(id); // 자리가 났으니 다음 대기 항목을 시작한다.
        ctx.request_repaint();
    }
}
