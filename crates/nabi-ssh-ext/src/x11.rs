//! X11 포워딩: 세션 채널에 X11을 요청하고, 서버가 여는 x11 채널을 로컬 X 서버로 릴레이.

use nabi_proto::{SshAuth, SshParams};
use russh::client::{self, AuthResult, Handle, Msg, Session};
use russh::Channel;
use tokio::net::TcpStream;

/// 인입 x11 채널을 로컬 X 서버(x_host:x_port)로 릴레이하는 핸들러.
/// 호스트키 검증은 SSH 터미널과 같은 `ClientHandler`에 위임한다.
pub struct X11Fwd {
    x_host: String,
    x_port: u16,
    hostkey: nabi_ssh::handler::ClientHandler,
}

impl client::Handler for X11Fwd {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        self.hostkey.check_server_key(key).await
    }

    async fn server_channel_open_x11(
        &mut self,
        channel: Channel<Msg>,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let host = self.x_host.clone();
        let port = self.x_port;
        tokio::spawn(async move {
            if let Ok(mut tcp) = TcpStream::connect((host.as_str(), port)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
            }
        });
        Ok(())
    }
}

/// X11 포워딩을 요청한다. 서버가 여는 x11 채널을 로컬 X 서버로 전달한다.
/// 반환 핸들이 살아있는 동안 포워딩이 유지된다.
pub async fn request_x11_forward(
    params: SshParams,
    x_host: String,
    x_port: u16,
) -> Result<Handle<X11Fwd>, String> {
    let config = std::sync::Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let handler = X11Fwd {
        x_host,
        x_port,
        hostkey: nabi_ssh::handler::ClientHandler::new(
            params.host.clone(),
            params.port,
            nabi_config::StorageLayout::resolve().known_hosts,
            None,
        ),
    };
    let mut handle = client::connect(config, (params.host.as_str(), params.port), handler)
        .await
        .map_err(|e| e.to_string())?;

    match &params.auth {
        SshAuth::Password(pw) => {
            let r = handle
                .authenticate_password(&params.user, pw)
                .await
                .map_err(|e| e.to_string())?;
            if !matches!(r, AuthResult::Success) {
                return Err("SSH 인증 실패".into());
            }
        }
        _ => return Err("X11: 비밀번호 인증만 지원".into()),
    }

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| e.to_string())?;
    channel
        .request_x11(false, false, "MIT-MAGIC-COOKIE-1", "0".repeat(32), 0)
        .await
        .map_err(|e| e.to_string())?;
    Ok(handle)
}
