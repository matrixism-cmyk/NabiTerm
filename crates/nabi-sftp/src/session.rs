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
    let config = Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
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
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
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
