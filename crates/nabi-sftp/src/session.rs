//! SFTP 연결 수립(별도 연결, 비밀번호·키 파일 인증).
//!
//! 호스트키는 SSH 터미널과 **동일한** 검증기(nabi-ssh `ClientHandler`)를 쓴다 —
//! 같은 호스트인데 터미널만 MITM 보호되고 SFTP는 무방비이던 문제를 없앤다.

use crate::fs::SftpFs;
use nabi_proto::{SshAuth, SshParams};
use nabi_ssh::handler::ClientHandler;
use nabi_ssh::verify::HostKeyVerifier;
use russh::client::{self, AuthResult};
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
    // 점프 호스트(ProxyJump, D2)가 있으면 경유, 아니면 직접 연결. jump 핸들은 터널 유지용.
    // 점프 호스트도 목적지와 똑같이 호스트키를 검증한다(경유지가 MITM 지점이 되지 않게).
    let (handle, jump) = if let Some(j) = &params.jump {
        let jc = client::connect(config.clone(), (j.host.as_str(), j.port), handler(&j.host, j.port));
        let mut jh = tokio::time::timeout(std::time::Duration::from_secs(15), jc)
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
        let mut handle = tokio::time::timeout(std::time::Duration::from_secs(15), connect)
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
    // 업로드 파이프라인 깊이를 기본(8)보다 올린다. SFTP는 요청/응답이라 미결 요청 수가 곧
    // 처리량이다(요청수 × 청크 ÷ RTT). OpenSSH sftp 클라이언트도 기본 64를 쓴다.
    // max_packet_len은 서버가 VERSION에서 알려주는 값과 min으로 협상되므로 상한만 크게 둔다.
    let cfg = russh_sftp::client::Config {
        max_concurrent_writes: 64,
        ..Default::default()
    };
    let sftp = russh_sftp::client::SftpSession::new_with_config(channel.into_stream(), cfg)
        .await
        .map_err(|e| e.to_string())?;

    Ok(SftpFs::new(sftp, handle, jump))
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
