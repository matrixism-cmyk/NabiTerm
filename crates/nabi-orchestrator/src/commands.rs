//! UI Command 처리.

use crate::pane_registry::{PaneRuntime, SharedPanes};
use crate::sftp::{sftp_request, spawn_sftp, SftpConns, SftpReq};
use crate::spawn_pane::connect_ssh_pane;
use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_proto::{Command, Event};
use nabi_types::PaneId;
use std::collections::HashMap;
use tokio::runtime::Handle;

/// 한 Command를 처리한다.
#[allow(clippy::too_many_arguments)]
pub fn handle_command(
    cmd: Command,
    rt: &Handle,
    state: &mut HashMap<PaneId, PaneRuntime>,
    sftp: &mut SftpConns,
    forwards: &mut HashMap<u64, tokio::task::JoinHandle<()>>,
    panes: &SharedPanes,
    out_tx: &Sender<(PaneId, Bytes)>,
    event_tx: &Sender<Event>,
    verifier: &std::sync::Arc<crate::hostkey::OrchVerifier>,
) {
    match cmd {
        // SpawnLocalPane은 액터 루프가 가로채 병렬 스폰한다(actor.rs).
        Command::SpawnLocalPane { .. } => {}
        Command::HostKeyDecision { id, accept } => verifier.resolve(id, accept),
        // trzsz 결정·취소: 세션이 낸 회신 바이트를 그 pane으로 그대로 흘려보낸다.
        Command::TrzszDecide(d) => {
            if let Some(rt) = state.get_mut(&d.pane) {
                let reply = rt.trzsz.decide(&d, event_tx);
                if !reply.is_empty() {
                    let _ = rt.transport.write(&reply);
                }
            }
        }
        Command::TrzszCancel { pane } => {
            if let Some(rt) = state.get_mut(&pane) {
                let reply = rt.trzsz.cancel(pane, event_tx);
                if !reply.is_empty() {
                    let _ = rt.transport.write(&reply);
                }
            }
        }
        Command::ConnectSsh {
            params,
            size,
            scrollback,
            encoding,
            stats_secs,
            reply_seq,
        } => {
            connect_ssh_pane(
                rt, params, size, scrollback, &encoding, state, panes, out_tx, event_tx,
                verifier.clone(), stats_secs, reply_seq,
            );
        }
        Command::WriteInput { pane, data } => {
            if let Some(p) = state.get_mut(&pane) {
                let _ = p.transport.write(&data);
            }
        }
        Command::Resize { pane, size } => {
            if let Some(p) = state.get_mut(&pane) {
                let _ = p.transport.resize(size);
            }
            if let Some(view) = crate::pane_registry::panes_read(panes).get(&pane) {
                crate::pane_registry::model_lock(&view.model).resize(size);
            }
        }
        Command::Broadcast { panes, data } => {
            // 지정된 pane에만 쓴다(과거: 창 ID를 무시하고 전체 pane에 씀 → 의도치 않은 명령 실행).
            for id in &panes {
                if let Some(p) = state.get_mut(id) {
                    let _ = p.transport.write(&data);
                }
            }
        }
        Command::ClosePane { pane } => {
            state.remove(&pane);
            crate::pane_registry::panes_write(panes).remove(&pane);
            let _ = event_tx.send(Event::PaneExited { pane, code: None });
        }
        Command::SetEncoding { pane, label } => {
            if let Some(rt) = state.get_mut(&pane) {
                rt.set_encoding(&label);
            }
        }
        Command::SftpConnect {
            id,
            params,
            ftp,
            limit_kbps,
            parallel,
        } => spawn_sftp(id, params, ftp, limit_kbps, parallel, rt, sftp, event_tx, verifier.clone()),
        Command::SftpList { id, path } => sftp_request(id, SftpReq::List(path), sftp),
        Command::SftpListTree { id, root, seq } => sftp_request(id, SftpReq::ListTree { root, seq }, sftp),
        Command::SftpDownload {
            id,
            xfer,
            remote,
            local,
            resume,
        } => sftp_request(id, SftpReq::Download { xfer, remote, local, resume }, sftp),
        Command::SftpUpload { id, xfer, local, remote } => {
            sftp_request(id, SftpReq::Upload { xfer, local, remote }, sftp)
        }
        Command::SftpRemove { id, path } => sftp_request(id, SftpReq::Remove(path), sftp),
        Command::SftpRename { id, from, to } => {
            sftp_request(id, SftpReq::Rename { from, to }, sftp)
        }
        Command::SftpMkdir { id, path } => sftp_request(id, SftpReq::Mkdir(path), sftp),
        Command::SftpTouch { id, path } => sftp_request(id, SftpReq::Touch(path), sftp),
        Command::SftpCancel { id } => crate::sftp::sftp_cancel(id, sftp),
        Command::SftpCancelXfer { id, xfer } => crate::sftp::sftp_cancel_xfer(id, xfer, sftp),
        Command::SftpChmod { id, path, mode } => {
            sftp_request(id, SftpReq::Chmod { path, mode }, sftp)
        }
        Command::SftpSearch { id, root, needle } => {
            sftp_request(id, SftpReq::Search { root, needle }, sftp)
        }
        Command::SftpDirSize { id, path } => {
            sftp_request(id, SftpReq::DirSize(path), sftp)
        }
        Command::SftpFreeSpace { id, path } => sftp_request(id, SftpReq::FreeSpace(path), sftp),
        Command::SftpPreview { id, path, max } => {
            sftp_request(id, SftpReq::Preview { path, max }, sftp)
        }
        Command::SftpChmodRecursive { id, path, mode } => {
            sftp_request(id, SftpReq::ChmodRec { path, mode }, sftp)
        }
        Command::SftpDownloadDir { id, xfer, remote, local } => {
            sftp_request(id, SftpReq::DownloadDir { xfer, remote, local }, sftp)
        }
        Command::SftpDownloadDirSync { id, remote, local, done } => {
            sftp_request(id, SftpReq::DownloadDirSync { remote, local, done }, sftp)
        }
        Command::SftpUploadDir { id, xfer, local, remote } => {
            sftp_request(id, SftpReq::UploadDir { xfer, local, remote }, sftp)
        }
        Command::SftpClose { id } => sftp_request(id, SftpReq::Close, sftp),
        Command::StartLocalForward {
            id,
            params,
            remote_host,
            remote_port,
        } => {
            let ev = event_tx.clone();
            let target = format!("{remote_host}:{remote_port}");
            let h = rt.spawn(async move {
                match nabi_ssh_ext::start_local_forward(params, remote_host, remote_port).await {
                    Ok(p) => {
                        let message = format!("127.0.0.1:{p} \u{2192} {target}");
                        let _ = ev.send(Event::ForwardStarted { id, message });
                        std::future::pending::<()>().await; // 중지(abort)될 때까지 유지.
                    }
                    Err(message) => {
                        let _ = ev.send(Event::Error { message });
                    }
                }
            });
            forwards.insert(id, h);
        }
        Command::StartDynamicForward { id, params } => {
            let ev = event_tx.clone();
            let h = rt.spawn(async move {
                match nabi_ssh_ext::start_dynamic_forward(params).await {
                    Ok(p) => {
                        let message = format!("127.0.0.1:{p} (SOCKS5 -D)");
                        let _ = ev.send(Event::ForwardStarted { id, message });
                        std::future::pending::<()>().await;
                    }
                    Err(message) => {
                        let _ = ev.send(Event::Error { message });
                    }
                }
            });
            forwards.insert(id, h);
        }
        Command::StartX11Forward { id, params } => {
            let ev = event_tx.clone();
            let h = rt.spawn(async move {
                // 로컬 X 서버는 DISPLAY :0 = 127.0.0.1:6000 관례(VcXsrv/Xming 등).
                match nabi_ssh_ext::x11::request_x11_forward(params, "127.0.0.1".into(), 6000).await {
                    Ok(_handle) => {
                        let _ = ev.send(Event::ForwardStarted { id, message: "X11 → 127.0.0.1:6000 (-X)".into() });
                        std::future::pending::<()>().await; // 핸들 유지(드롭되면 포워딩 종료).
                    }
                    Err(message) => {
                        let _ = ev.send(Event::Error { message });
                    }
                }
            });
            forwards.insert(id, h);
        }
        Command::StartRemoteForward {
            id,
            params,
            remote_port,
        } => {
            let ev = event_tx.clone();
            let h = rt.spawn(async move {
                match nabi_ssh_ext::start_remote_forward(params, remote_port, "127.0.0.1".into(), remote_port)
                    .await
                {
                    Ok(_handle) => {
                        let message = format!("server:{remote_port} \u{2192} 127.0.0.1:{remote_port} (-R)");
                        let _ = ev.send(Event::ForwardStarted { id, message });
                        std::future::pending::<()>().await; // 핸들 유지(드롭되면 포워딩 종료).
                    }
                    Err(message) => {
                        let _ = ev.send(Event::Error { message });
                    }
                }
            });
            forwards.insert(id, h);
        }
        Command::StopForward { id } => {
            if let Some(h) = forwards.remove(&id) {
                h.abort();
            }
        }
        // 후속 마일스톤에서 처리.
        Command::ReloadConfig | Command::Shutdown => {}
    }
}
