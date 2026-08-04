//! 로컬 포트 포워딩(-L) + 포워딩군 공용 연결 헬퍼.

use nabi_proto::{SshAuth, SshParams};
use russh::client::{self, AuthResult};
use std::sync::Arc;
use tokio::net::TcpListener;

/// 포워딩군 공용 클라이언트 핸들러(호스트키 임시 수락).
pub struct Fwd;

impl client::Handler for Fwd {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// SSH에 연결·인증해 Handle을 반환한다(포워딩군 공용). 비밀번호 인증.
pub(crate) async fn connect_authed(params: &SshParams) -> Result<client::Handle<Fwd>, String> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (params.host.as_str(), params.port), Fwd)
        .await
        .map_err(|e| e.to_string())?;

    let result = match &params.auth {
        SshAuth::Password(pw) => handle
            .authenticate_password(&params.user, pw)
            .await
            .map_err(|e| e.to_string())?,
        _ => return Err("포워딩: 현재 비밀번호 인증만 지원".into()),
    };
    if !matches!(result, AuthResult::Success) {
        return Err("SSH 인증 실패".into());
    }
    Ok(handle)
}

/// 로컬 포트 포워딩(-L): 임시 로컬 포트의 각 연결을 (remote_host:remote_port)로 포워딩.
pub async fn start_local_forward(
    params: SshParams,
    remote_host: String,
    remote_port: u16,
) -> Result<u16, String> {
    let handle = Arc::new(connect_authed(&params).await?);
    let listener = TcpListener::bind(("127.0.0.1", 0u16))
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let h = handle.clone();
            let rh = remote_host.clone();
            tokio::spawn(async move {
                if let Ok(channel) = h
                    .channel_open_direct_tcpip(rh, remote_port as u32, "127.0.0.1", 0)
                    .await
                {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                }
            });
        }
    });
    Ok(port)
}
