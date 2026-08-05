//! SFTP 전송 워커 풀 — 전송마다 **자기 연결**을 쓰게 해서 실제로 동시에 보낸다.
//!
//! 지금까지 연결 하나에 액터 하나였고 요청을 FIFO로 처리했다. 그래서 설정의
//! "동시 전송 개수"를 2 이상으로 올려도 의미가 없었다 — 두 번째 전송은 첫 번째가
//! 끝날 때까지 큐에서 기다렸다. 목록·삭제 같은 짧은 작업까지 큰 파일 하나에 막혔다.
//!
//! 워커는 **첫 작업이 올 때** 연결을 만든다(안 쓰면 열지 않는다). 서버의 세션 한도를
//! 생각해 상한을 둔다(OpenSSH 기본 MaxSessions 10).

use crate::connsftp::Conn;
use crossbeam_channel::Sender;
use nabi_proto::{Event, SftpId, SshParams};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// 워커가 맡는 전송 작업.
pub(crate) enum Job {
    Download { xfer: u64, remote: String, local: String, resume: u64 },
    Upload { xfer: u64, local: String, remote: String },
    DownloadDir { xfer: u64, remote: String, local: String },
    UploadDir { xfer: u64, local: String, remote: String },
}

impl Job {
    fn xfer(&self) -> u64 {
        match self {
            Job::Download { xfer, .. }
            | Job::Upload { xfer, .. }
            | Job::DownloadDir { xfer, .. }
            | Job::UploadDir { xfer, .. } => *xfer,
        }
    }

    /// 완료 이벤트에 실을 이름(사용자가 큐에서 보는 것과 같은 대상).
    fn name(&self) -> &str {
        match self {
            Job::Download { local, .. } | Job::DownloadDir { local, .. } => local,
            Job::Upload { remote, .. } | Job::UploadDir { remote, .. } => remote,
        }
    }
}

/// xfer → 취소 플래그. 액터 밖(`sftp_cancel_xfer`)에서도 건드리므로 공유한다.
pub type Flags = Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>;

/// 새 플래그 맵(연결을 만들 때 하나씩).
pub fn new_flags() -> Flags {
    Arc::new(Mutex::new(HashMap::new()))
}

/// 이 연결의 전송을 모두 취소한다.
pub fn cancel_all(flags: &Flags) {
    if let Ok(m) = flags.lock() {
        for f in m.values() {
            f.store(true, Ordering::Relaxed);
        }
    }
}

/// 전송 하나만 취소한다(큐에서 그 줄의 ✕).
pub fn cancel_one(flags: &Flags, xfer: u64) {
    if let Ok(m) = flags.lock() {
        if let Some(f) = m.get(&xfer) {
            f.store(true, Ordering::Relaxed);
        }
    }
}

struct Worker {
    tx: mpsc::UnboundedSender<Job>,
    /// 이 워커에 들어가 있는 작업 수(대기 포함) — 어디로 보낼지 고르는 기준.
    load: Arc<AtomicUsize>,
}

/// 한 SFTP 연결(=한 원격 패널)에 딸린 전송 워커들.
pub(crate) struct Pool {
    id: SftpId,
    params: SshParams,
    limit_kbps: u32,
    max: usize,
    ev: Sender<Event>,
    flags: Flags,
    workers: Vec<Worker>,
}

impl Pool {
    pub(crate) fn new(
        id: SftpId,
        params: SshParams,
        limit_kbps: u32,
        max: usize,
        ev: Sender<Event>,
        flags: Flags,
    ) -> Self {
        // 서버 세션 한도(OpenSSH 기본 MaxSessions 10)를 넘지 않게 넉넉히 자른다.
        Self { id, params, limit_kbps, max: max.clamp(1, 4), ev, flags, workers: Vec::new() }
    }

    /// 작업을 놀고 있는 워커에게 준다. 없으면 상한까지 새 워커를 만들고,
    /// 그것도 안 되면 가장 한가한 워커 뒤에 붙인다.
    pub(crate) fn dispatch(&mut self, job: Job) {
        let idle = self.workers.iter().position(|w| w.load.load(Ordering::Relaxed) == 0);
        let at = match idle {
            Some(i) => i,
            None if self.workers.len() < self.max => self.spawn_worker(),
            None => self
                .workers
                .iter()
                .enumerate()
                .min_by_key(|(_, w)| w.load.load(Ordering::Relaxed))
                .map(|(i, _)| i)
                .unwrap_or(0),
        };
        let w = &self.workers[at];
        w.load.fetch_add(1, Ordering::Relaxed);
        if w.tx.send(job).is_err() {
            w.load.fetch_sub(1, Ordering::Relaxed); // 워커가 죽었으면 부하만 되돌린다.
        }
    }

    fn spawn_worker(&mut self) -> usize {
        let (tx, rx) = mpsc::unbounded_channel::<Job>();
        let load = Arc::new(AtomicUsize::new(0));
        tokio::spawn(worker_loop(
            rx,
            self.id,
            self.params.clone(),
            self.limit_kbps,
            self.ev.clone(),
            self.flags.clone(),
            load.clone(),
        ));
        self.workers.push(Worker { tx, load });
        self.workers.len() - 1
    }
}

/// 워커 한 개: 첫 작업에서 연결을 만들고, 이후 작업을 순서대로 처리한다.
async fn worker_loop(
    mut rx: mpsc::UnboundedReceiver<Job>,
    id: SftpId,
    params: SshParams,
    limit_kbps: u32,
    ev: Sender<Event>,
    flags: Flags,
    load: Arc<AtomicUsize>,
) {
    let mut fs: Option<Conn> = None;
    while let Some(job) = rx.recv().await {
        let xfer = job.xfer();
        // 작업마다 새 플래그 — 앞 작업을 취소했다고 다음 작업이 죽으면 안 된다.
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut m) = flags.lock() {
            m.insert(xfer, cancel.clone());
        }
        if fs.is_none() {
            fs = crate::sftpretry::reconnect_sftp(&params, limit_kbps, cancel.clone()).await;
        }
        let res = match fs.as_mut() {
            Some(c) => {
                c.set_cancel(cancel.clone());
                run_job(c, &job, &params, limit_kbps, &cancel, id, &ev).await
            }
            None => Err("전송용 추가 연결을 열지 못했습니다".to_string()),
        };
        if let Ok(mut m) = flags.lock() {
            m.remove(&xfer);
        }
        load.fetch_sub(1, Ordering::Relaxed);
        let _ = ev.send(Event::SftpTransferDone {
            id,
            xfer,
            name: job.name().to_string(),
            ok: res.is_ok(),
            message: res.err().unwrap_or_default(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_job(
    fs: &mut Conn,
    job: &Job,
    params: &SshParams,
    limit_kbps: u32,
    cancel: &Arc<AtomicBool>,
    id: SftpId,
    ev: &Sender<Event>,
) -> Result<(), String> {
    match job {
        Job::Download { xfer, remote, local, resume } => {
            crate::sftpretry::run_download(
                fs, params, limit_kbps, cancel, id, *xfer, remote, local, *resume, ev,
            )
            .await
        }
        Job::Upload { xfer, local, remote } => {
            crate::sftpretry::run_upload(fs, params, limit_kbps, cancel, id, *xfer, local, remote, ev)
                .await
        }
        Job::DownloadDir { remote, local, .. } => fs.download_dir(remote, Path::new(local)).await,
        Job::UploadDir { local, remote, .. } => fs.upload_dir(Path::new(local), remote).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(max: usize) -> Pool {
        let (tx, _rx) = crossbeam_channel::unbounded();
        Pool::new(7, SshParams::password("h", 22, "u", "p"), 0, max, tx, new_flags())
    }

    /// 상한을 넘겨 연결을 만들지 않는다 — 서버 세션 한도를 넘기면 접속 자체가 거부된다.
    #[test]
    fn caps_worker_count() {
        let p = pool(99);
        assert_eq!(p.max, 4, "설정이 커도 4를 넘지 않는다");
        assert_eq!(pool(0).max, 1, "0이어도 최소 1");
    }

    /// 취소는 그 전송만 끊는다 — 같은 연결의 다른 전송까지 죽이면 안 된다.
    #[test]
    fn cancel_one_targets_single_transfer() {
        let f = new_flags();
        let (a, b) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        f.lock().unwrap().insert(1, a.clone());
        f.lock().unwrap().insert(2, b.clone());
        cancel_one(&f, 1);
        assert!(a.load(Ordering::Relaxed) && !b.load(Ordering::Relaxed));
        cancel_all(&f);
        assert!(b.load(Ordering::Relaxed), "전체 취소는 나머지도 끊는다");
    }

    /// 없는 xfer를 취소해도 조용히 넘어간다(이미 끝난 전송의 ✕를 눌러도 안전).
    #[test]
    fn cancel_unknown_is_noop() {
        let f = new_flags();
        cancel_one(&f, 42);
        assert!(f.lock().unwrap().is_empty());
    }
}
