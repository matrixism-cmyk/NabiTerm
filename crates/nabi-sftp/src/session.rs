//! SFTP 연결 수립(별도 연결, 비밀번호·키 파일 인증).

use crate::fs::SftpFs;
use nabi_proto::{SshAuth, SshParams};
use russh::client::{self, AuthResult};
use std::sync::Arc;

/// 호스트키 핸들러(구조 구현: 임시 수락). 실제 known_hosts 검증은 nabi-ssh 참조.
pub(crate) struct Handler;

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// SFTP 세션을 연다. 런타임 동작 확인엔 SSH 서버가 필요하다.
pub async fn connect_sftp(params: &SshParams) -> Result<SftpFs, String> {
    // 유휴 연결이 서버 타임아웃으로 끊기지 않도록 keepalive(30초마다, 3회 실패 시 종료).
    let config = Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    // 점프 호스트(ProxyJump, D2)가 있으면 경유, 아니면 직접 연결. jump 핸들은 터널 유지용.
    let (handle, jump) = if let Some(j) = &params.jump {
        let jc = client::connect(config.clone(), (j.host.as_str(), j.port), Handler);
        let mut jh = tokio::time::timeout(std::time::Duration::from_secs(15), jc)
            .await.map_err(|_| "점프 연결 시간 초과".to_string())?.map_err(|e| e.to_string())?;
        auth(&mut jh, j).await?;
        let ch = jh.channel_open_direct_tcpip(params.host.clone(), params.port as u32, "127.0.0.1", 0)
            .await.map_err(|e| e.to_string())?;
        let mut th = client::connect_stream(config, ch.into_stream(), Handler).await.map_err(|e| e.to_string())?;
        auth(&mut th, params).await?;
        (th, Some(jh))
    } else {
        let connect = client::connect(config, (params.host.as_str(), params.port), Handler);
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
