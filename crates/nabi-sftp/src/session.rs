//! SFTP 연결 수립(별도 연결, 비밀번호·키 파일 인증).
//!
//! 호스트키는 SSH 터미널과 **동일한** 검증기(nabi-ssh `ClientHandler`)를 쓴다 —
//! 같은 호스트인데 터미널만 MITM 보호되고 SFTP는 무방비이던 문제를 없앤다.

use crate::fs::SftpFs;
use crate::raw::{Feat, RawFs, POSIX_RENAME};
use nabi_proto::{SshAuth, SshParams};
use nabi_ssh::handler::ClientHandler;
use nabi_ssh::legacy::{connect_compat, ConnOpts};
use nabi_ssh::verify::HostKeyVerifier;
use russh::client::{self, AuthResult};
use russh_sftp::client::RawSftpSession;
use std::path::PathBuf;
use std::sync::Arc;

/// SFTP 연결에 쓰는 호스트키 핸들러(= SSH 터미널과 동일 구현).
pub(crate) type Handler = ClientHandler;

/// 물려받은 연결 — 목적지 핸들과(있다면) 점프 호스트 핸들.
///
/// 점프 핸들도 함께 받는다. **다만 그 근거는 시험이 아니라 판단이다** — 실서버로 확인해
/// 보니 목적지 핸들만 넘겨도 터널이 곧바로 끊기지는 않았다(SSH 라이브러리의 배경 태스크가
/// 세션을 붙들고 있다). 그러니 "빠뜨리면 끊긴다"고 단정하지 않는다.
///
/// 그래도 함께 받는 이유: 그 수명이 우리가 정하지 않은 구현 세부에 기대고 있고, 그 세부는
/// 예고 없이 바뀔 수 있다. 잡고 있는 값은 싸고, 놓쳤을 때의 값은 "가끔 끊기는 연결"이다.
pub type ReusedConn = nabi_ssh::conns::SshConn;

/// SFTP 세션을 연다.
///
/// `known_hosts`에 알려진 키면 수락, 미지 호스트는 `verifier`가 있으면 사용자에게 묻고
/// 없으면 TOFU 학습, **키가 바뀌었으면 거부**한다. 런타임 확인엔 SSH 서버가 필요하다.
pub async fn connect_sftp(
    params: &SshParams,
    known_hosts: PathBuf,
    verifier: Option<HostKeyVerifier>,
) -> Result<SftpFs, String> {
    connect_sftp_reusing(params, known_hosts, verifier, None).await
}

/// 같은 서버에 이미 붙어 있는 SSH 연결을 **그대로 쓴다**(배치 Y H5).
///
/// `reuse` 가 `Some` 이면 새 연결도 인증도 하지 않고 그 연결에 SFTP 채널만 더 연다.
/// 사용자에게는 비밀번호를 다시 묻지 않는 것으로, 서버에는 세션이 하나로 보인다.
///
/// `None` 이면 지금까지처럼 새로 연결한다 — 되돌릴 수 있게 남겨 둔 길이다(H6).
pub async fn connect_sftp_reusing(
    params: &SshParams,
    known_hosts: PathBuf,
    verifier: Option<HostKeyVerifier>,
    reuse: Option<ReusedConn>,
) -> Result<SftpFs, String> {
    if let Some(c) = reuse {
        // 채널 하나를 더 여는 것은 이미 증명된 길이다 — copyid(v0.1.478)가 SFTP 세션의
        // 연결에 exec 채널을 열고 있다. 반대 방향으로 하는 것뿐이다.
        return open_on(c.handle, c.jump).await;
    }
    let handler = |host: &str, port: u16| {
        ClientHandler::new(host.to_string(), port, known_hosts.clone(), verifier.clone())
    };
    // 유휴 연결이 서버 타임아웃으로 끊기지 않도록 keepalive(30초마다, 3회 실패 시 종료).
    //
    // 전송 속도 관련 두 가지를 기본값에서 바꾼다:
    // - nodelay: SFTP는 작은 요청/응답을 주고받는데 Nagle이 켜져 있으면 매 왕복에 지연이 붙는다.
    // - window_size: SSH 채널 창이 곧 처리량 상한이다(창 ÷ RTT). 기본 2MiB는 RTT 50ms에서
    //   약 41MB/s로 묶여, 요청을 아무리 파이프라이닝해도 그 위로 못 올라간다.
    let opts = ConnOpts { keepalive_secs: 30, nodelay: true, window_size: 16 * 1024 * 1024 };
    // 연결 제한시간에는 **호스트키 확인창을 읽는 시간**도 포함된다(핸드셰이크 안에서 기다린다).
    // 확인창이 뜰 수 있으면 넉넉히 주고, 자동 재접속(verifier 없음)은 짧게 유지한다.
    let limit = std::time::Duration::from_secs(if verifier.is_some() { 180 } else { 15 });
    // 점프 호스트(ProxyJump, D2)가 있으면 경유, 아니면 직접 연결. jump 핸들은 터널 유지용.
    // 점프 호스트도 목적지와 똑같이 호스트키를 검증한다(경유지가 MITM 지점이 되지 않게).
    // 옛 서버(OpenSSH 4.x 등)는 SHA-1 알고리즘만 내놓는다 — 협상이 깨지면 레거시로 한 번 더
    // 시도한다(nabi-ssh legacy.rs). SSH 터미널과 같은 규칙이라 한쪽만 붙는 일이 없다.
    let (handle, jump) = if let Some(j) = &params.jump {
        let (mut jh, _) = connect_compat(&opts, |cfg| {
            let h = handler(&j.host, j.port);
            async move {
                tokio::time::timeout(limit, client::connect(cfg, (j.host.as_str(), j.port), h))
                    .await
                    .map_err(|_| russh::Error::ConnectionTimeout)?
            }
        })
        .await
        .map_err(|e| e.to_string())?;
        auth(&mut jh, j).await?;
        let (mut th, _) = connect_compat(&opts, |cfg| {
            let h = handler(&params.host, params.port);
            let jref = &jh;
            async move {
                let ch = jref
                    .channel_open_direct_tcpip(params.host.clone(), params.port as u32, "127.0.0.1", 0)
                    .await?;
                client::connect_stream(cfg, ch.into_stream(), h).await
            }
        })
        .await
        .map_err(|e| e.to_string())?;
        auth(&mut th, params).await?;
        (th, Some(jh))
    } else {
        let (mut handle, _) = connect_compat(&opts, |cfg| {
            let h = handler(&params.host, params.port);
            async move {
                tokio::time::timeout(limit, client::connect(cfg, (params.host.as_str(), params.port), h))
                    .await
                    .map_err(|_| russh::Error::ConnectionTimeout)?
            }
        })
        .await
        .map_err(|e| e.to_string())?;
        auth(&mut handle, params).await?;
        (handle, None)
    };

    open_on(std::sync::Arc::new(handle), jump.map(std::sync::Arc::new)).await
}

/// 연결 위에 SFTP 서브시스템 채널을 열고 `SftpFs` 를 만든다.
///
/// 새로 붙은 연결과 물려받은 연결이 **같은 코드**를 지나가게 한다. 둘을 따로 적으면
/// 언젠가 한쪽에만 고침이 들어간다.
async fn open_on(
    handle: std::sync::Arc<client::Handle<Handler>>,
    jump: Option<std::sync::Arc<client::Handle<Handler>>>,
) -> Result<SftpFs, String> {
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| e.to_string())?;
    let raw = open_raw(channel.into_stream()).await?;
    Ok(SftpFs::new(raw, handle, jump))
}

/// SFTP 서브시스템 스트림 위에 raw 세션을 열고 서버 확장을 감지한다.
///
/// 고수준 `SftpSession`을 쓰지 않는 이유는 raw.rs 참고(확장·파이프라이닝 접근 불가).
pub(crate) async fn open_raw<S>(stream: S) -> Result<RawFs, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    // 요청 하나의 응답 대기 시간. 기본 10초는 파이프라인으로 수 MB를 띄워 두는 우리 방식과
    // 맞지 않는다 — 느린 회선에서 아직 순서를 기다리는 요청이 멀쩡히 시간 초과된다.
    let cfg = russh_sftp::client::Config { request_timeout_secs: 120, ..Default::default() };
    let mut session = RawSftpSession::new_with_config(stream, cfg);
    let version = session.init().await.map_err(|e| e.to_string())?;
    let has = |name: &str, ver: &str| version.extensions.get(name).is_some_and(|v| v == ver);
    let mut feat = Feat {
        posix_rename: has(POSIX_RENAME, "1"),
        fsync: has("fsync@openssh.com", "1"),
        statvfs: has("statvfs@openssh.com", "2"),
        ..Default::default()
    };
    // 청크 크기를 추측하지 않고 서버가 알려준 한도를 쓴다(limits@openssh.com).
    if has("limits@openssh.com", "1") {
        if let Ok(l) = session.limits().await {
            feat.read_len = Some(l.max_read_len);
            feat.write_len = Some(l.max_write_len);
            session.set_limits(l.into());
        }
    }
    Ok(RawFs::new(session, feat))
}

/// 핸들에 비밀번호/키 파일 인증을 수행한다(직접·점프 공용).
async fn auth(handle: &mut client::Handle<Handler>, params: &SshParams) -> Result<(), String> {
    let result = match &params.auth {
        SshAuth::Password(pw) => handle.authenticate_password(&params.user, pw).await.map_err(|e| e.to_string())?,
        SshAuth::KeyFile { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref()).map_err(|e| e.to_string())?;
            let with_hash = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None);
            handle.authenticate_publickey(&params.user, with_hash).await.map_err(|e| e.to_string())?
        }
        // 에이전트 인증은 서명을 ssh-agent에 맡기므로 개인키가 이 프로세스로 오지 않는다.
        SshAuth::Agent => {
            nabi_ssh::agent::authenticate_agent(handle, &params.user)
                .await
                .map_err(|e| format!("{}: {e}", nabi_i18n::trc("net.sftp.agent")))?;
            AuthResult::Success
        }
        SshAuth::None => return Err(nabi_i18n::trc("net.sftp.noauth").into()),
    };
    matches!(result, AuthResult::Success).then_some(()).ok_or_else(|| nabi_i18n::trc("net.sftp.authfail").to_string())
}
