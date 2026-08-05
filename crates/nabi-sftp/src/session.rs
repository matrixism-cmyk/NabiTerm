//! SFTP 연결 수립(별도 연결, 비밀번호·키 파일 인증).
//!
//! 호스트키는 SSH 터미널과 **동일한** 검증기(nabi-ssh `ClientHandler`)를 쓴다 —
//! 같은 호스트인데 터미널만 MITM 보호되고 SFTP는 무방비이던 문제를 없앤다.

use crate::fs::SftpFs;
use crate::raw::{Feat, RawFs, POSIX_RENAME};
use nabi_proto::{SshAuth, SshParams};
use nabi_ssh::handler::ClientHandler;
use nabi_ssh::verify::HostKeyVerifier;
use russh::client::{self, AuthResult};
use russh_sftp::client::RawSftpSession;
use std::path::PathBuf;
use std::sync::Arc;

/// SFTP 연결에 쓰는 호스트키 핸들러(= SSH 터미널과 동일 구현).
pub(crate) type Handler = ClientHandler;

/// SFTP 세션을 연다.
///
/// `known_hosts`에 알려진 키면 수락, 미지 호스트는 `verifier`가 있으면 사용자에게 묻고
/// 없으면 TOFU 학습, **키가 바뀌었으면 거부**한다. 런타임 확인엔 SSH 서버가 필요하다.
pub async fn connect_sftp(
    params: &SshParams,
    known_hosts: PathBuf,
    verifier: Option<HostKeyVerifier>,
) -> Result<SftpFs, String> {
    let handler = |host: &str, port: u16| {
        ClientHandler::new(host.to_string(), port, known_hosts.clone(), verifier.clone())
    };
    // 유휴 연결이 서버 타임아웃으로 끊기지 않도록 keepalive(30초마다, 3회 실패 시 종료).
    //
    // 전송 속도 관련 두 가지를 기본값에서 바꾼다:
    // - nodelay: SFTP는 작은 요청/응답을 주고받는데 Nagle이 켜져 있으면 매 왕복에 지연이 붙는다.
    // - window_size: SSH 채널 창이 곧 처리량 상한이다(창 ÷ RTT). 기본 2MiB는 RTT 50ms에서
    //   약 41MB/s로 묶여, 요청을 아무리 파이프라이닝해도 그 위로 못 올라간다.
    let config = Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        nodelay: true,
        window_size: 16 * 1024 * 1024,
        ..Default::default()
    });
    // 연결 제한시간에는 **호스트키 확인창을 읽는 시간**도 포함된다(핸드셰이크 안에서 기다린다).
    // 확인창이 뜰 수 있으면 넉넉히 주고, 자동 재접속(verifier 없음)은 짧게 유지한다.
    let limit = std::time::Duration::from_secs(if verifier.is_some() { 180 } else { 15 });
    // 점프 호스트(ProxyJump, D2)가 있으면 경유, 아니면 직접 연결. jump 핸들은 터널 유지용.
    // 점프 호스트도 목적지와 똑같이 호스트키를 검증한다(경유지가 MITM 지점이 되지 않게).
    let (handle, jump) = if let Some(j) = &params.jump {
        let jc = client::connect(config.clone(), (j.host.as_str(), j.port), handler(&j.host, j.port));
        let mut jh = tokio::time::timeout(limit, jc)
            .await.map_err(|_| "점프 연결 시간 초과".to_string())?.map_err(|e| e.to_string())?;
        auth(&mut jh, j).await?;
        let ch = jh.channel_open_direct_tcpip(params.host.clone(), params.port as u32, "127.0.0.1", 0)
            .await.map_err(|e| e.to_string())?;
        let mut th = client::connect_stream(config, ch.into_stream(), handler(&params.host, params.port))
            .await.map_err(|e| e.to_string())?;
        auth(&mut th, params).await?;
        (th, Some(jh))
    } else {
        let connect = client::connect(config, (params.host.as_str(), params.port), handler(&params.host, params.port));
        let mut handle = tokio::time::timeout(limit, connect)
            .await.map_err(|_| "연결 시간 초과".to_string())?.map_err(|e| e.to_string())?;
        auth(&mut handle, params).await?;
        (handle, None)
    };

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
        SshAuth::None => return Err("SFTP: 인증 정보가 없습니다".into()),
    };
    matches!(result, AuthResult::Success).then_some(()).ok_or_else(|| "SFTP 인증 실패".to_string())
}
