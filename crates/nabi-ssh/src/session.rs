//! SSH 연결 수립 + 입출력 펌프 태스크.

use crate::channel::{SshChannel, SshInput};
use crate::handler::ClientHandler;
use crate::legacy::ConnOpts;
use crate::params::{SshAuth, SshParams};
use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_proto::Event;
use nabi_types::{GridSize, PaneId};
use russh::client;
use russh::ChannelMsg;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

/// SSH keepalive 간격(초) — 0이면 끄기. 앱이 설정에서 갱신, 연결 시 읽는다(ServerAliveInterval 대응).
pub static SSH_KEEPALIVE_SECS: AtomicU64 = AtomicU64::new(30);
use tokio::runtime::Handle;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

/// 서버 통계 폴링 설정: (이벤트 전송자, 폴링 주기). None이면 폴링 안 함.
pub type StatsPoll = Option<(Sender<Event>, Duration)>;

/// SSH 연결을 비동기로 시작하고 입력용 ByteChannel을 돌려준다.
/// `on_close`는 세션 종료 시 한 번 호출된다(None=정상 종료, Some=오류 메시지).
#[allow(clippy::too_many_arguments)]
pub fn connect(
    rt: &Handle,
    pane: PaneId,
    params: SshParams,
    size: GridSize,
    out_tx: Sender<(PaneId, Bytes)>,
    known_hosts: PathBuf,
    verifier: Option<crate::verify::HostKeyVerifier>,
    on_close: Box<dyn FnOnce(Option<String>) + Send>,
    stats: StatsPoll,
) -> SshChannel {
    let (in_tx, in_rx) = unbounded_channel::<SshInput>();
    let out = out_tx.clone();
    rt.spawn(async move {
        let res = run(pane, params, size, out.clone(), in_rx, known_hosts, verifier, stats).await;
        crate::kexinfo::clear(pane); // 배지 잔상 방지.
        let err = res.err().map(|e| e.to_string());
        if let Some(e) = &err {
            let _ = out.send((pane, Bytes::from(format!("\r\n[{}: {e}]\r\n", nabi_i18n::trc("net.ssh.err")))));
        }
        on_close(err);
    });
    SshChannel::new(in_tx)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    pane: PaneId,
    params: SshParams,
    size: GridSize,
    out_tx: Sender<(PaneId, Bytes)>,
    mut in_rx: UnboundedReceiver<SshInput>,
    known_hosts: PathBuf,
    verifier: Option<crate::verify::HostKeyVerifier>,
    stats: StatsPoll,
) -> Result<(), russh::Error> {
    // 유휴 연결이 서버 타임아웃으로 끊기지 않도록 keepalive(설정값 초, 0=끄기, 3회 실패 시 종료).
    let secs = SSH_KEEPALIVE_SECS.load(std::sync::atomic::Ordering::Relaxed);
    let opts = ConnOpts { keepalive_secs: secs, ..Default::default() };
    // 직접 연결 또는 점프 호스트(ProxyJump) 경유. _jump은 터널 유지를 위해 살려둔다.
    // kex 슬롯: 목적지 연결의 협상 결과(KEX·암호)를 받아 pane 레지스트리에 기록(PQ 배지).
    let kex_slot = crate::kexinfo::new_slot();
    let (handle, _jump, old) = open_authed(&params, opts, known_hosts, verifier, kex_slot.clone()).await?;
    if let Some(info) = kex_slot.lock().ok().and_then(|s| s.clone()) {
        crate::kexinfo::set(pane, info);
    }
    if old {
        // 조용히 넘어가면 사용자는 자기가 SHA-1로 붙었는지 알 수 없다.
        let msg = format!("\r\n[{}]\r\n", nabi_i18n::trc("net.legacy.notice"));
        let _ = out_tx.send((pane, Bytes::from(msg)));
    }

    let channel = handle.channel_open_session().await?;
    // ssh-agent 포워딩(세션에서 켠 경우에만). 원격에서 다시 git/ssh를 쓸 때 내 키로 서명한다.
    // 실패해도 세션은 그대로 연다 — 포워딩이 안 될 뿐 로그인은 이미 끝났다.
    if params.agent_forward {
        if let Err(e) = channel.agent_forward(false).await {
            let msg = format!("
[{}: {e}]
", nabi_i18n::trc("ssh.agentfwd.failed"));
            let _ = out_tx.send((pane, Bytes::from(msg)));
        }
    }
    channel
        .request_pty(false, "xterm-256color", size.cols() as u32, size.rows() as u32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;
    // 통계 폴러를 별도 채널/태스크로 실행(대화형 출력과 분리). handle는 Arc로 공유.
    let handle = Arc::new(handle);
    let poller = stats.map(|(tx, interval)| {
        tokio::spawn(crate::stats::poll_loop(pane, handle.clone(), tx, interval))
    });
    let result = pump(pane, channel, out_tx, &mut in_rx).await;
    if let Some(p) = poller {
        p.abort(); // 대화형 세션 종료 시 폴러도 정리(연결 누수 방지).
    }
    result
}

/// 인증된 target 핸들을 얻는다. params.jump가 있으면 점프 호스트를 경유(direct-tcpip 터널 위
/// 두 번째 SSH 세션). 반환=(target, jump 유지용 핸들). jump 핸들이 드롭되면 터널이 끊기므로 보관.
///
/// 반환의 마지막 `bool`은 레거시(SHA-1) 알고리즘으로 붙었는지.
#[allow(clippy::type_complexity)]
async fn open_authed(
    params: &SshParams,
    opts: ConnOpts,
    known_hosts: PathBuf,
    verifier: Option<crate::verify::HostKeyVerifier>,
    kex_slot: crate::kexinfo::KexSlot,
) -> Result<
    (client::Handle<ClientHandler>, Option<client::Handle<ClientHandler>>, bool),
    russh::Error,
> {
    // 죽은 호스트에서 무한 대기하지 않도록 제한을 둔다. 다만 이 시간에는 **호스트키 확인창을
    // 사용자가 읽는 시간**도 포함된다 — 지문을 확인하고 신뢰를 누르면 이미 시간이 지나 있었다.
    // 확인창이 뜰 수 있는 경우(verifier 있음)에만 넉넉하게 준다.
    let d15 = std::time::Duration::from_secs(if verifier.is_some() { 180 } else { 15 });
    if let Some(jump) = &params.jump {
        // 옛 서버 대응(legacy.rs): 협상이 안 되면 SHA-1 목록으로 한 번만 다시 붙는다.
        let (mut jhandle, old_j) = crate::legacy::connect_compat(&opts, |cfg| {
            let h = ClientHandler::new(jump.host.clone(), jump.port, known_hosts.clone(), verifier.clone());
            async move {
                tokio::time::timeout(d15, client::connect(cfg, (jump.host.as_str(), jump.port), h))
                    .await
                    .map_err(|_| russh::Error::ConnectionTimeout)?
            }
        })
        .await?;
        authenticate(&mut jhandle, jump).await?;
        // 터널 위의 목적지도 따로 협상한다 — 재시도 때는 채널부터 다시 연다.
        let (mut thandle, old_t) = crate::legacy::connect_compat(&opts, |cfg| {
            let h = ClientHandler::new(params.host.clone(), params.port, known_hosts.clone(), verifier.clone())
                .with_kex_slot(kex_slot.clone())
                // 포워딩은 **목적지에만** 켠다. 점프 호스트는 통로일 뿐인데 거기까지 켜면
                // 경유지 관리자에게도 내 키를 내주는 셈이 된다.
                .with_agent_forward(params.agent_forward);
            let jh = &jhandle;
            async move {
                let ch = jh
                    .channel_open_direct_tcpip(params.host.clone(), params.port as u32, "127.0.0.1", 0)
                    .await?;
                client::connect_stream(cfg, ch.into_stream(), h).await
            }
        })
        .await?;
        authenticate(&mut thandle, params).await?;
        Ok((thandle, Some(jhandle), old_j || old_t))
    } else {
        let (mut handle, old) = crate::legacy::connect_compat(&opts, |cfg| {
            let h = ClientHandler::new(params.host.clone(), params.port, known_hosts.clone(), verifier.clone())
                .with_kex_slot(kex_slot.clone())
                // 포워딩은 **목적지에만** 켠다. 점프 호스트는 통로일 뿐인데 거기까지 켜면
                // 경유지 관리자에게도 내 키를 내주는 셈이 된다.
                .with_agent_forward(params.agent_forward);
            async move {
                tokio::time::timeout(d15, client::connect(cfg, (params.host.as_str(), params.port), h))
                    .await
                    .map_err(|_| russh::Error::ConnectionTimeout)?
            }
        })
        .await?;
        authenticate(&mut handle, params).await?;
        Ok((handle, None, old))
    }
}

/// 출력 버스로 보낸다. 버스가 가득 차면 **블록하지 않고** 잠깐 양보 후 재시도한다.
///
/// 출력 버스는 폭주 방지를 위해 상한이 있는데, 여기서 crossbeam의 블로킹 `send`를 쓰면
/// tokio 워커 스레드가 통째로 멈춘다(같은 런타임의 다른 SSH 세션·SFTP 전송까지 정지).
/// 반환 false = 수신측 종료.
async fn send_output(out_tx: &Sender<(PaneId, Bytes)>, pane: PaneId, data: Bytes) -> bool {
    let mut item = (pane, data);
    loop {
        match out_tx.try_send(item) {
            Ok(()) => return true,
            Err(crossbeam_channel::TrySendError::Full(back)) => {
                item = back;
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => return false,
        }
    }
}

async fn pump(
    pane: PaneId,
    mut channel: russh::Channel<client::Msg>,
    out_tx: Sender<(PaneId, Bytes)>,
    in_rx: &mut UnboundedReceiver<SshInput>,
) -> Result<(), russh::Error> {
    loop {
        tokio::select! {
            msg = channel.wait() => match msg {
                Some(ChannelMsg::Data { data }) => {
                    if !send_output(&out_tx, pane, Bytes::copy_from_slice(&data)).await {
                        break;
                    }
                }
                Some(ChannelMsg::ExtendedData { data, .. }) => {
                    if !send_output(&out_tx, pane, Bytes::copy_from_slice(&data)).await {
                        break;
                    }
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            },
            input = in_rx.recv() => match input {
                Some(SshInput::Data(d)) => channel.data(&d[..]).await?,
                Some(SshInput::Resize(c, r)) => {
                    channel.window_change(c as u32, r as u32, 0, 0).await?;
                }
                None => break,
            },
        }
    }
    Ok(())
}

async fn authenticate(
    handle: &mut client::Handle<ClientHandler>,
    params: &SshParams,
) -> Result<(), russh::Error> {
    use russh::client::AuthResult;
    let result = match &params.auth {
        SshAuth::None => return Err(russh::Error::NotAuthenticated),
        SshAuth::Password(pw) => handle.authenticate_password(&params.user, pw).await?,
        SshAuth::KeyFile { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref())?;
            let with_hash = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None);
            handle.authenticate_publickey(&params.user, with_hash).await?
        }
        SshAuth::Agent => {
            crate::agent::authenticate_agent(handle, &params.user)
                .await
                .map_err(|_| russh::Error::NotAuthenticated)?;
            AuthResult::Success
        }
    };
    if matches!(result, AuthResult::Success) {
        Ok(())
    } else {
        Err(russh::Error::NotAuthenticated)
    }
}
