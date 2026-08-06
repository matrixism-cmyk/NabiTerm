//! SFTP 전송 큐 — 항목 모델·상태 기계·디스패치. UI는 sftpqueue, 동작 처리는 sftpqact.

use crate::app::NabiApp;
use crate::sftppath::join_path;
use nabi_proto::Command;

impl NabiApp {
    /// 원격 항목을 탐색기로 드래그-아웃(파일=가상파일, 폴더=재귀 다운로드). DoDragDrop 블로킹.
    pub(crate) fn start_remote_drag(&self, name: &str, size: u64, is_dir: bool, id: nabi_proto::SftpId) {
        let remote = join_path(&self.sftp.path, name);
        let tx = self.orch.cmd_tx.clone();
        if is_dir {
            crate::windndfolder::drag_out_remote_dir(name.to_string(), remote, id, tx);
        } else {
            let file = crate::windndvirt::RemoteFile { name: name.to_string(), size, remote_path: remote };
            crate::windndvirt::drag_out_remote(file, id, tx);
        }
    }

    /// 로컬 경로를 현재 원격 디렉터리의 하위 폴더(subfolder)로 업로드한다(폴더 행에 드롭).
    pub(crate) fn upload_into(&mut self, subfolder: &str, local: std::path::PathBuf) {
        let Some(id) = self.sftp.id else { return };
        let Some(fname) = local.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        let remote = join_path(&join_path(&self.sftp.path, subfolder), &fname);
        let size = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
        let is_dir = local.is_dir();
        let local = local.to_string_lossy().into_owned();
        self.push_xfer(format!("{subfolder}/{fname}"), true, size, move |xfer| {
            if is_dir {
                Command::SftpUploadDir { id, xfer, local, remote }
            } else {
                Command::SftpUpload { id, xfer, local, remote }
            }
        });
    }

    /// 로컬 경로 하나를 현재 원격 디렉터리로 업로드한다(파일/폴더 자동, 앱 내 DnD·드롭 공용).
    /// OS 클립보드(CF_HDROP)의 로컬 파일들을 현재 SFTP 폴더로 업로드(붙여넣기, FileZilla식).
    pub(crate) fn sftp_paste_upload(&mut self) {
        if self.sftp.id.is_none() {
            return;
        }
        for src in crate::winclip::paste_paths() {
            self.upload_local_path(src);
        }
    }

    pub(crate) fn upload_local_path(&mut self, local: std::path::PathBuf) {
        let Some(id) = self.sftp.id else { return };
        let Some(fname) = local.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        let remote = join_path(&self.sftp.path, &fname);
        let is_dir = local.is_dir();
        let size = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
        let local = local.to_string_lossy().into_owned();
        self.push_xfer(fname, true, size, move |xfer| {
            if is_dir {
                Command::SftpUploadDir { id, xfer, local, remote }
            } else {
                Command::SftpUpload { id, xfer, local, remote }
            }
        });
    }
}

/// 큐 항목의 상태. 대기 상태가 있어야 순서 변경·일시정지가 의미를 갖는다 —
/// 명령을 곧바로 전부 보내 버리면 통제할 여지가 없다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum XferState {
    /// 차례를 기다리는 중. 순서 변경·일시정지·제거가 자유롭다.
    Waiting,
    /// 사용자가 멈춰 둠 — 차례가 와도 시작하지 않는다.
    Paused,
    /// 서버로 보냈고 진행 중.
    Running,
    Done,
    Failed,
}

impl XferState {
    /// 끝난 항목(성공·실패). 큐를 더 이상 점유하지 않는다.
    pub(crate) fn finished(self) -> bool {
        matches!(self, XferState::Done | XferState::Failed)
    }
}

/// 한 건의 파일 전송 상태.
pub(crate) struct Transfer {
    /// 이 항목의 고유 식별자. 진행률·완료 이벤트가 이 값으로 항목을 지목하므로
    /// 큐 순서가 바뀌거나(재시도) 다른 파일 작업이 끼어들어도 엉뚱한 행이 갱신되지 않는다.
    pub xfer: u64,
    pub name: String,
    pub up: bool,
    pub size: u64,
    pub bytes: u64,
    pub state: XferState,
    /// 전송을 **시작한** 시각(큐에 넣은 시각이 아니다 — 대기 시간이 속도에 섞이지 않게).
    pub started: std::time::Instant,
    /// 이 항목을 실행할 명령. 시작·재시도 모두 같은 명령을 쓴다
    /// (다운로드는 남은 `.filepart`에서 이어받으므로 재시도해도 처음부터 받지 않는다).
    pub cmd: Option<Command>,
    /// 실패 사유(항목별 오류 툴팁, 성공 시 빈 문자열).
    pub err: String,
}

impl Transfer {
    pub(crate) fn new(xfer: u64, name: String, up: bool, size: u64) -> Self {
        Self {
            xfer,
            name,
            up,
            size,
            bytes: 0,
            state: XferState::Waiting,
            started: std::time::Instant::now(),
            cmd: None,
            err: String::new(),
        }
    }

    /// 진행 중인가(속도·집계 대상).
    pub(crate) fn running(&self) -> bool {
        self.state == XferState::Running
    }
}

/// 전송 큐에 항목이 없는 내부 전송(원격 편집 임시파일·드래그아웃 등)의 식별자.
///
/// 이 값으로 온 진행률·완료 이벤트는 큐에서 매칭되는 항목이 없어 조용히 무시된다 —
/// 의도된 동작이다(사용자에게 보여줄 큐 행이 애초에 없다).
pub(crate) const XFER_NONE: u64 = 0;

impl NabiApp {
    /// 큐에 전송 항목을 넣는다(바로 보내지 않는다). 모든 업/다운로드 경로 공용.
    ///
    /// 식별자를 여기서 발급해 `make`로 명령에 주입한다 — 큐 항목과 명령이 항상 같은 id를
    /// 갖도록 강제해, 호출부가 실수로 어긋나게 만들 수 없다.
    pub(crate) fn push_xfer(
        &mut self,
        name: String,
        up: bool,
        size: u64,
        make: impl FnOnce(u64) -> Command,
    ) {
        self.xfer_seq += 1;
        let xfer = self.xfer_seq;
        let mut t = Transfer::new(xfer, name, up, size);
        t.cmd = Some(make(xfer));
        self.sftp.transfers.push(t);
        if let Some(id) = self.sftp.id {
            self.pump_transfers(id);
        }
    }

    /// 대기 중인 항목을 동시 실행 한도까지 시작한다.
    ///
    /// 실패한 항목은 끝난 것으로 치므로 큐를 막지 않는다. 한도를 넘겨 보내지 않는 덕분에
    /// "대기" 상태가 실재하고, 그래서 순서 변경·일시정지가 의미를 갖는다.
    pub(crate) fn pump_transfers(&mut self, id: nabi_proto::SftpId) {
        let limit = self.config.terminal.max_parallel_transfers.clamp(1, 4) as usize;
        let mut send = Vec::new();
        if let Some(p) = self.remote_panel_mut(id) {
            let mut running = p.transfers.iter().filter(|t| t.running()).count();
            for t in p.transfers.iter_mut() {
                if running >= limit {
                    break;
                }
                if t.state != XferState::Waiting {
                    continue;
                }
                let Some(cmd) = t.cmd.clone() else { continue };
                t.state = XferState::Running;
                t.started = std::time::Instant::now();
                t.bytes = 0;
                t.err.clear();
                send.push(cmd);
                running += 1;
            }
        }
        for c in send {
            self.orch.send(c);
        }
    }
}

/// 전송 큐 집계(순수, H1) — (bytes, size, speed) 튜플들을 합산해 (총bytes, 총size, 총speed B/s).
/// FileZilla식 큐 헤더(전체 진행률·합산 속도)에 쓴다. 시간 의존을 분리해 단위테스트가 가능하다.
pub(crate) fn xfer_totals(items: &[(u64, u64, u64)]) -> (u64, u64, u64) {
    items.iter().fold((0, 0, 0), |(b, s, sp), &(tb, ts, tsp)| (b + tb, s + ts, sp + tsp))
}

/// 현재 속도로 남은 바이트를 보내는 데 걸릴 예상 초(시작 직후·완료·정보부족이면 None).
pub(crate) fn eta_secs(bytes: u64, size: u64, elapsed: f64) -> Option<u64> {
    if bytes == 0 || elapsed <= 0.0 || bytes >= size {
        return None;
    }
    let speed = bytes as f64 / elapsed;
    (speed > 0.0).then(|| ((size - bytes) as f64 / speed) as u64)
}

/// 초를 사람이 읽는 짧은 표기로(예: 45s, 1m23s, 2h05m).
pub(crate) fn human_secs(s: u64) -> String {
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// 큐가 **지금은 더 움직이지 않는가** — 진행 중도 대기도 없다(일시정지는 있어도 된다).
///
/// 목록 갱신 판단에 "전부 끝났나"를 쓰면 안 된다. 일시정지 항목 하나가 남으면 그 상태가
/// 영원히 유지돼 목록이 끝내 갱신되지 않는다 — 사용자가 올린 파일이 보이지 않는다.
/// 정지한 항목은 재개될 때 다시 낡음 표시가 붙으므로 여기서 기다릴 이유가 없다.
pub(crate) fn settled(transfers: &[Transfer]) -> bool {
    !transfers.iter().any(|t| t.running() || t.state == XferState::Waiting)
}

/// 원격 목록을 다시 받아야 하는가 — **큐가 다 빈 순간에 한 번만**.
///
/// 업로드마다 받으면 동시 전송 중에 목록이 계속 갈아엎힌다(24개를 올리면 24번).
/// `stale`은 소비된다(한 번 새로 고치면 다시 낡을 때까지 안 받는다).
pub(crate) fn take_refresh(drained: bool, stale: &mut bool) -> bool {
    drained && std::mem::take(stale)
}

#[cfg(test)]
mod tests {
    use super::{eta_secs, human_secs, settled, take_refresh, xfer_totals, Transfer, XferState, XFER_NONE};

    /// 큐 항목은 id로 지목해야 한다 — 위치로 찾으면 다른 파일 작업이 끼어들거나
    /// 재시도로 순서가 바뀔 때 엉뚱한 행이 갱신된다(과거 "첫 미완료 항목" 방식의 버그).
    #[test]
    fn transfers_are_addressed_by_id_not_position() {
        let mut q = [
            Transfer::new(10, "big.iso".into(), false, 1000),
            Transfer::new(11, "a.txt".into(), false, 10),
        ];
        q[0].state = XferState::Running;
        q[1].state = XferState::Running;
        // 두 번째 항목이 먼저 끝났다고 알려와도, 첫 항목(진행 중)은 건드리지 않아야 한다.
        if let Some(t) = q.iter_mut().find(|t| t.xfer == 11) {
            t.state = XferState::Done;
        }
        assert!(q[0].running(), "진행 중이던 큰 파일이 완료 처리되면 안 된다");
        assert!(q[1].state.finished());

        // 진행률도 id로 귀속된다.
        if let Some(t) = q.iter_mut().find(|t| t.xfer == 10) {
            t.bytes = 512;
        }
        assert_eq!(q[0].bytes, 512);
        assert_eq!(q[1].bytes, 0, "다른 항목의 진행률이 새면 안 된다");
    }

    /// 큐에 없는 내부 전송(원격 편집·드래그아웃)은 매칭되는 항목이 없어 조용히 무시된다.
    #[test]
    fn internal_transfers_match_nothing() {
        let q = [Transfer::new(1, "x".into(), false, 1)];
        assert!(q.iter().all(|t| t.xfer != XFER_NONE), "내부 전송 id는 큐와 겹치지 않는다");
    }

    /// 새 항목은 대기 상태로 들어간다 — 곧바로 보내면 순서 변경·일시정지가 불가능하다.
    #[test]
    fn new_items_start_waiting() {
        let t = Transfer::new(1, "x".into(), false, 1);
        assert_eq!(t.state, XferState::Waiting);
        assert!(!t.running() && !t.state.finished());
    }

    #[test]
    fn totals_sum_components() {
        let items = [(10u64, 100u64, 5u64), (40, 100, 15), (0, 50, 0)];
        assert_eq!(xfer_totals(&items), (50, 250, 20)); // bytes·size·speed 각각 합산.
        assert_eq!(xfer_totals(&[]), (0, 0, 0));
    }

    #[test]
    fn eta_and_format() {
        // 1초에 500/1000 → 남은 500, 속도 500/s → 약 1초.
        assert_eq!(eta_secs(500, 1000, 1.0), Some(1));
        assert_eq!(eta_secs(0, 1000, 1.0), None); // 시작 직후.
        assert_eq!(eta_secs(1000, 1000, 1.0), None); // 완료.
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(83), "1m23s");
        assert_eq!(human_secs(7505), "2h05m");
    }

    /// 목록 갱신은 큐가 빈 순간 한 번만 — 항목마다 받으면 동시 전송 중 목록이 요동친다.
    #[test]
    fn refresh_only_once_when_queue_drains() {
        let mut stale = false;
        assert!(!take_refresh(true, &mut stale), "올린 게 없으면 갱신도 없다");
        stale = true;
        assert!(!take_refresh(false, &mut stale), "아직 남았으면 기다린다");
        assert!(stale, "표시는 그대로 남아 있어야 한다");
        assert!(take_refresh(true, &mut stale), "다 끝나면 한 번 받는다");
        assert!(!take_refresh(true, &mut stale), "두 번은 받지 않는다");
    }

    /// 일시정지 항목이 남아도 목록은 갱신돼야 한다(정지는 "끝남"이 아니지만 "멈춤"이다).
    #[test]
    fn paused_item_does_not_block_refresh() {
        let mut v = vec![Transfer::new(1, "a".into(), true, 10), Transfer::new(2, "b".into(), true, 10)];
        v[0].state = XferState::Done;
        v[1].state = XferState::Paused;
        assert!(settled(&v), "정지만 남았으면 더 움직이지 않는다");
        assert!(!v.iter().all(|t| t.state.finished()), "전부 끝난 것은 아니다");
        v[1].state = XferState::Waiting;
        assert!(!settled(&v), "대기가 있으면 아직 움직일 것이 남았다");
        v[1].state = XferState::Running;
        assert!(!settled(&v));
    }
}
