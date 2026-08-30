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

impl Job {
    /// 워커가 처리하지 못한 작업을 액터의 단일 연결 경로로 돌려보내기 위한 변환.
    fn into_req(self) -> crate::sftp::SftpReq {
        use crate::sftp::SftpReq as R;
        match self {
            Job::Download { xfer, remote, local, resume } => R::Download { xfer, remote, local, resume },
            Job::Upload { xfer, local, remote } => R::Upload { xfer, local, remote },
            Job::DownloadDir { xfer, remote, local } => R::DownloadDir { xfer, remote, local },
            Job::UploadDir { xfer, local, remote } => R::UploadDir { xfer, local, remote },
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
    /// 추가 연결을 열지 못한 서버 — 이후로는 풀을 쓰지 않고 주 연결로만 보낸다.
    ///
    /// `MaxSessions`가 1~2인 서버, fail2ban/`MaxStartups`로 재로그인을 막는 서버가 실제로
    /// 있다. 그런 곳에서 병렬 전송을 넣은 뒤로 **예전에는 되던 전송이 실패**하면 안 된다.
    degraded: Arc<AtomicBool>,
    /// 워커가 못 맡은 작업을 액터에게 돌려보내는 통로(주 연결로 재실행).
    back: mpsc::UnboundedSender<crate::sftp::SftpReq>,
}

impl Pool {
    pub(crate) fn new(
        id: SftpId,
        params: SshParams,
        limit_kbps: u32,
        max: usize,
        ev: Sender<Event>,
        flags: Flags,
        back: mpsc::UnboundedSender<crate::sftp::SftpReq>,
    ) -> Self {
        // 서버 세션 한도(OpenSSH 기본 MaxSessions 10)를 넘지 않게 넉넉히 자른다.
        Self {
            id, params, limit_kbps, max: max.clamp(1, 4), ev, flags,
            workers: Vec::new(), degraded: Arc::new(AtomicBool::new(false)), back,
        }
    }

    /// 추가 연결이 안 되는 서버로 판명됐는가 — 그렇다면 액터가 주 연결로 처리해야 한다.
    pub(crate) fn degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
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
        let ctx = WorkerCtx {
            id: self.id,
            params: self.params.clone(),
            limit_kbps: self.limit_kbps,
            ev: self.ev.clone(),
            flags: self.flags.clone(),
            degraded: self.degraded.clone(),
            back: self.back.clone(),
        };
        tokio::spawn(worker_loop(rx, ctx, load.clone()));
        self.workers.push(Worker { tx, load });
        self.workers.len() - 1
    }
}

/// 워커가 공유하는 것들(인자 수를 줄이려고 묶는다 — 전부 풀 수명 동안 고정).
struct WorkerCtx {
    id: SftpId,
    params: SshParams,
    limit_kbps: u32,
    ev: Sender<Event>,
    flags: Flags,
    degraded: Arc<AtomicBool>,
    back: mpsc::UnboundedSender<crate::sftp::SftpReq>,
}

/// 워커 한 개: 첫 작업에서 연결을 만들고, 이후 작업을 순서대로 처리한다.
async fn worker_loop(
    mut rx: mpsc::UnboundedReceiver<Job>,
    w: WorkerCtx,
    load: Arc<AtomicUsize>,
) {
    let WorkerCtx { id, params, limit_kbps, ev, flags, degraded, back } = w;
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
        let Some(c) = fs.as_mut() else {
            // 추가 연결 실패 = 이 서버에서는 병렬을 못 쓴다. 실패로 끝내지 말고
            // 주 연결로 돌려보낸다(예전에는 되던 전송이니 사용자에겐 그대로 성공해야 한다).
            degraded.store(true, Ordering::Relaxed);
            if let Ok(mut m) = flags.lock() {
                m.remove(&xfer);
            }
            load.fetch_sub(1, Ordering::Relaxed);
            let _ = back.send(job.into_req());
            continue;
        };
        c.set_cancel(cancel.clone());
        let res = run_job(c, &job, &params, limit_kbps, &cancel, id, &ev).await;
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

/// 누적 바이트를 진행 이벤트로 흘려보내는 싱크(파일 전송과 같은 임계로 합친다).
pub(crate) fn progress_sink(id: SftpId, xfer: u64, ev: &Sender<Event>) -> impl FnMut(u64) + Send + '_ {
    let mut last = 0u64;
    move |total| {
        if total.saturating_sub(last) >= crate::sftpretry::PROGRESS_STEP {
            last = total;
            let _ = ev.send(Event::SftpProgress { id, xfer, bytes: total });
        }
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
        // 폴더도 파일과 같은 방식으로 진행률을 보낸다 — 큐 줄이 끝날 때까지 멈춰 보이지 않게.
        Job::DownloadDir { xfer, remote, local } => {
            let mut p = progress_sink(id, *xfer, ev);
            // 건너뛴 것이 있으면 실패로 말한다 — 전송은 끝났지만 다 받은 것은 아니다.
            fs.download_dir(remote, Path::new(local), &mut p).await.and_then(skipped_err)
        }
        Job::UploadDir { xfer, local, remote } => {
            let mut p = progress_sink(id, *xfer, ev);
            fs.upload_dir(Path::new(local), remote, &mut p).await
        }
    }
}

/// 폴더를 다 받았는가. 건너뛴 것이 있으면 **성공이라고 말하지 않는다.**
///
/// 건너뛴 것은 서버가 준 이름이 받는 폴더를 벗어나는 경우다(`..` 나 절대 경로).
/// 그런 이름을 그대로 쓰면 엉뚱한 자리에 파일이 써지므로 뺐는데, 그 사실을 숨기면
/// 사용자는 다 받은 줄 알고 원본을 지운다.
///
/// 두 자리(큐 실행기·단발 요청)가 같은 말을 하도록 여기 한 곳에 둔다.
pub(crate) fn skipped_err(skipped: usize) -> Result<(), String> {
    match skipped {
        0 => Ok(()),
        n => Err(format!("sftp.skipped.unsafe:{n}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(max: usize) -> Pool {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let (back, _br) = tokio::sync::mpsc::unbounded_channel();
        Pool::new(7, SshParams::password("h", 22, "u", "p"), 0, max, tx, new_flags(), back)
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
