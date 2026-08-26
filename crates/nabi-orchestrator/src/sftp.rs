//! SFTP 연결 액터(연결당 tokio 태스크) — 원격 파일 브라우저 데이터 경로.
//!
//! `connect_sftp`로 연결 후 mpsc로 List/Close 요청을 받아 처리하고,
//! 결과를 `Event`로 UI에 보낸다. 라이브 동작은 인프로세스 SFTP 서버로 검증한다.

use crate::connsftp::Conn;
use crate::sftpev::{op_done, to_entry, transfer_done};
use crossbeam_channel::Sender;
use nabi_proto::{Event, SftpId, SshAuth, SshParams};
use nabi_sftp::connect_sftp;
use std::collections::HashMap;
use std::path::Path;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

/// 액터로 보내는 요청.
pub enum SftpReq {
    List(String),
    /// 여유 공간 묻기.
    FreeSpace(String),
    /// 앞부분만 읽기(미리보기).
    Preview { path: String, max: usize },
    Download { xfer: u64, remote: String, local: String, resume: u64 },
    Upload { xfer: u64, local: String, remote: String },
    Remove(String),
    Rename { from: String, to: String },
    Mkdir(String),
    Touch(String),
    DownloadDir { xfer: u64, remote: String, local: String },
    DownloadDirSync { remote: String, local: String, done: std::sync::mpsc::Sender<bool> },
    UploadDir { xfer: u64, local: String, remote: String },
    /// 서버에서 명령 한 줄 실행.
    Exec { cmd: String },
    /// 서버 안에서 복사(큐 항목 xfer 로 진행률·완료를 짝짓는다).
    Copy { xfer: u64, from: String, to: String, dir: bool },
    Chmod { path: String, mode: u32 },
    ChmodRec { path: String, mode: u32 },
    Search { root: String, needle: String },
    /// 동기화 계획용 파일 트리 수집(상대경로·크기·mtime) — seq 상관 회신.
    ListTree { root: String, seq: u64 },
    DirSize(String),
    Close,
}

/// 한 SFTP 연결의 바깥 손잡이(요청 보내기 + 취소).
pub struct ConnHandle {
    tx: mpsc::UnboundedSender<SftpReq>,
    /// 주 연결의 취소 플래그(목록·삭제 등 액터가 직접 하는 작업용).
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// 워커 풀의 전송별 취소 플래그(xfer → 플래그).
    flags: crate::sftppool::Flags,
}

/// SftpId → 연결 손잡이.
pub type SftpConns = HashMap<SftpId, ConnHandle>;

/// SFTP 연결 액터를 띄운다(connect → 요청 루프).
#[allow(clippy::too_many_arguments)]
pub fn spawn_sftp(
    id: SftpId,
    params: SshParams,
    ftp: bool,
    limit_kbps: u32,
    parallel: u32,
    rt: &Handle,
    conns: &mut SftpConns,
    event_tx: &Sender<Event>,
    verifier: std::sync::Arc<crate::hostkey::OrchVerifier>,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<SftpReq>();
    // 워커가 못 맡은 작업을 다시 이 액터로 돌려보낼 통로(주 연결 폴백).
    let back = tx.clone();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flags = crate::sftppool::new_flags();
    conns.insert(id, ConnHandle { tx, cancel: cancel.clone(), flags: flags.clone() });
    let ev = event_tx.clone();
    // SSH 터미널과 같은 known_hosts·확인 모달을 쓴다(SFTP만 무방비이던 문제 해소).
    let known_hosts = nabi_config::StorageLayout::resolve().known_hosts;
    rt.spawn(async move {
        // 재시도·재접속용 취소 플래그 클론(set_cancel가 원본을 소비하기 전에 확보).
        let cancel_retry = cancel.clone();
        let conn = if ftp {
            let pass = if let SshAuth::Password(p) = &params.auth { p.clone() } else { String::new() };
            nabi_ftp::connect_ftp(&params.host, params.port, &params.user, &pass)
                .await
                .map(Conn::Ftp)
        } else {
            connect_sftp(&params, known_hosts.clone(), Some(verifier.clone()))
                .await
                .map(|mut f| {
                    f.set_limit(limit_kbps as u64 * 1024); // KB/s → bytes/s.
                    f.set_cancel(cancel); // 외부 취소 플래그 연결.
                    Conn::Sftp(f)
                })
        };
        // 동시 전송이 2 이상이면 전송을 별도 연결의 워커로 넘긴다(1이면 예전처럼 이 연결에서).
        let mut pool = (parallel > 1 && !ftp).then(|| {
            crate::sftppool::Pool::new(id, params.clone(), limit_kbps, parallel as usize, ev.clone(), flags, back)
        });
        let mut fs = match conn {
            Ok(c) => {
                let _ = ev.send(Event::SftpConnected { id });
                c
            }
            Err(message) => { let _ = ev.send(Event::SftpError { id, message }); return; }
        };
        while let Some(req) = rx.recv().await {
            match req {
                SftpReq::ListTree { root, seq } => {
                    let mut files = Vec::new();
                    match fs.list_tree(&root, "", &mut files).await {
                        Ok(()) => { files.sort(); let _ = ev.send(Event::SftpTree { id, seq, files }); }
                        Err(message) => { let _ = ev.send(Event::SftpError { id, message }); }
                    }
                }
                SftpReq::List(path) => match fs.list_dir(&path).await {
                    Ok(items) => {
                        let entries = items.into_iter().map(to_entry).collect();
                        let _ = ev.send(Event::SftpListing { id, path, entries });
                    }
                    Err(message) => { let _ = ev.send(Event::SftpError { id, message }); }
                },
                SftpReq::Download { xfer, remote, local, resume } if pool.as_ref().is_some_and(|p| !p.degraded()) => {
                    let p = pool.as_mut().expect("위에서 확인함");
                    p.dispatch(crate::sftppool::Job::Download { xfer, remote, local, resume });
                }
                SftpReq::Upload { xfer, local, remote } if pool.as_ref().is_some_and(|p| !p.degraded()) => {
                    let p = pool.as_mut().expect("위에서 확인함");
                    p.dispatch(crate::sftppool::Job::Upload { xfer, local, remote });
                }
                SftpReq::DownloadDir { xfer, remote, local } if pool.as_ref().is_some_and(|p| !p.degraded()) => {
                    let p = pool.as_mut().expect("위에서 확인함");
                    p.dispatch(crate::sftppool::Job::DownloadDir { xfer, remote, local });
                }
                SftpReq::UploadDir { xfer, local, remote } if pool.as_ref().is_some_and(|p| !p.degraded()) => {
                    let p = pool.as_mut().expect("위에서 확인함");
                    p.dispatch(crate::sftppool::Job::UploadDir { xfer, local, remote });
                }
                SftpReq::Download { xfer, remote, local, resume } => {
                    // 연결 오류 시 재접속+이어받기로 재시도(S2 #14/#15).
                    let res = crate::sftpretry::run_download(
                        &mut fs, &params, limit_kbps, &cancel_retry, id, xfer, &remote, &local, resume, &ev,
                    )
                    .await;
                    let _ = ev.send(transfer_done(id, xfer, &local, res));
                }
                SftpReq::Upload { xfer, local, remote } => {
                    let res = crate::sftpretry::run_upload(
                        &mut fs, &params, limit_kbps, &cancel_retry, id, xfer, &local, &remote, &ev,
                    )
                    .await;
                    let _ = ev.send(transfer_done(id, xfer, &remote, res));
                }
                SftpReq::Remove(path) => {
                    // 재귀 삭제(비어있지 않은 디렉터리도 안전하게 제거).
                    let res = fs.remove_recursive(&path).await;
                    let _ = ev.send(op_done(id, &path, res));
                }
                SftpReq::Rename { from, to } => {
                    let res = fs.rename(&from, &to).await;
                    let _ = ev.send(op_done(id, &to, res));
                }
                SftpReq::Mkdir(path) => {
                    let res = fs.mkdir(&path).await;
                    let _ = ev.send(op_done(id, &path, res));
                }
                SftpReq::Touch(path) => {
                    let res = fs.touch(&path).await;
                    let _ = ev.send(op_done(id, &path, res));
                }
                SftpReq::DownloadDir { xfer, remote, local } => {
                    let mut p = crate::sftppool::progress_sink(id, xfer, &ev);
                    let res = fs.download_dir(&remote, Path::new(&local), &mut p).await;
                    let _ = ev.send(transfer_done(id, xfer, &local, res));
                }
                SftpReq::DownloadDirSync { remote, local, done } => {
                    // 가상 폴더 드래그-아웃: 완료를 done 채널로(UI 이벤트 루프 비경유).
                    let mut noop = |_: u64| {};
                    let res = fs.download_dir(&remote, Path::new(&local), &mut noop).await;
                    let _ = done.send(res.is_ok());
                }
                SftpReq::UploadDir { xfer, local, remote } => {
                    let mut p = crate::sftppool::progress_sink(id, xfer, &ev);
                    let res = fs.upload_dir(Path::new(&local), &remote, &mut p).await;
                    let _ = ev.send(transfer_done(id, xfer, &remote, res));
                }
                SftpReq::Exec { cmd } => {
                    // 상한은 넉넉히 두되 무한은 아니다 — 서버가 기가바이트를 뱉을 수 있다.
                    let r = fs.exec_remote(&cmd, 256 * 1024).await;
                    let (out, code) = match r {
                        Ok(v) => v,
                        Err(e) => (e, None),
                    };
                    let _ = ev.send(Event::SftpExecDone { id, cmd, out, code });
                }
                SftpReq::Copy { xfer, from, to, dir } => {
                    // 진행률은 전송과 같은 통로로 보낸다 — 큐에서 다른 전송과 나란히 보인다.
                    let ev2 = ev.clone();
                    let mut p = move |n: u64| {
                        let _ = ev2.send(Event::SftpProgress { id, xfer, bytes: n });
                    };
                    let res = match dir {
                        true => fs.copy_dir_remote(&from, &to, &mut p).await,
                        false => fs.copy_remote(&from, &to, &mut p).await.map(|_| ()),
                    };
                    let _ = ev.send(transfer_done(id, xfer, &to, res));
                }
                SftpReq::Chmod { path, mode } => {
                    let res = fs.chmod(&path, mode).await;
                    let _ = ev.send(op_done(id, &path, res));
                }
                SftpReq::ChmodRec { path, mode } => {
                    let res = fs.chmod_recursive(&path, mode).await;
                    let _ = ev.send(op_done(id, &path, res));
                }
                SftpReq::Search { root, needle } => {
                    let results = fs.search(&root, &needle, 500).await;
                    let _ = ev.send(Event::SftpSearchResults { id, results });
                }
                SftpReq::FreeSpace(path) => {
                    let free = fs.free_space(&path).await;
                    let _ = ev.send(Event::SftpFreeSpace { id, free });
                }
                SftpReq::Preview { path, max } => {
                    let (data, more, err) = match fs.preview(&path, max).await {
                        Ok((d, m)) => (d, m, None),
                        Err(e) => (Vec::new(), false, Some(e)),
                    };
                    let _ = ev.send(Event::SftpPreview { id, path, data, more, err });
                }
                SftpReq::DirSize(path) => {
                    let (files, dirs, bytes) = fs.dir_stats(&path).await;
                    let _ = ev.send(Event::SftpDirSize { id, path, files, dirs, bytes });
                }
                SftpReq::Close => break,
            }
        }
    });
}

/// 목록/종료 요청을 해당 액터로 전달한다(Close면 맵에서 제거).
pub fn sftp_request(id: SftpId, req: SftpReq, conns: &mut SftpConns) {
    let closing = matches!(req, SftpReq::Close);
    if let Some(h) = conns.get(&id) {
        let _ = h.tx.send(req);
    }
    if closing {
        conns.remove(&id);
    }
}

/// 이 연결의 전송을 **모두** 취소한다(주 연결 + 워커 풀).
pub fn sftp_cancel(id: SftpId, conns: &SftpConns) {
    if let Some(h) = conns.get(&id) {
        h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::sftppool::cancel_all(&h.flags);
    }
}

/// 전송 **하나만** 취소한다 — 큐에서 그 줄의 ✕를 눌렀을 때.
///
/// 예전에는 이것도 연결 전체를 끊어, 동시 전송 중 하나를 지우면 나머지도 같이 죽었다.
pub fn sftp_cancel_xfer(id: SftpId, xfer: u64, conns: &SftpConns) {
    if let Some(h) = conns.get(&id) {
        crate::sftppool::cancel_one(&h.flags, xfer);
    }
}

#[cfg(test)]
mod tests {
    use super::to_entry;
    use nabi_fs::{FileEntry, FileKind};

    #[test]
    fn maps_dir_and_file() {
        let d = to_entry(FileEntry {
            name: "docs".into(),
            kind: FileKind::Dir,
            size: 0,
            mode: 0o755,
            mtime: 0,
        });
        assert!(d.is_dir && d.name == "docs" && d.mode == 0o755);
        let f = to_entry(FileEntry {
            name: "a.txt".into(),
            kind: FileKind::File,
            size: 7,
            mode: 0o644,
            mtime: 1700,
        });
        assert!(!f.is_dir && f.size == 7 && f.mode == 0o644 && f.mtime == 1700);
        assert!(!f.is_link); // 일반 파일은 링크 아님.
        let l = to_entry(FileEntry { name: "ln".into(), kind: FileKind::Symlink, size: 0, mode: 0o777, mtime: 0 });
        assert!(l.is_link && !l.is_dir); // 심볼릭 링크 표시.
    }
}
