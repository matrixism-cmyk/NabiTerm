//! 원격 포트 포워딩(-R): 서버 포트의 인바운드 연결을 로컬 대상으로 전달.

use nabi_proto::{SshAuth, SshParams};
use russh::client::{self, AuthResult, Handle, Msg, Session};
use russh::Channel;
use tokio::net::TcpStream;

/// 인바운드 forwarded-tcpip 채널을 로컬 대상으로 릴레이하는 클라이언트 핸들러.
/// 호스트키 검증은 SSH 터미널과 같은 `ClientHandler`에 위임한다.
pub struct RemoteFwd {
    local_host: String,
    local_port: u16,
    hostkey: nabi_ssh::handler::ClientHandler,
}

impl client::Handler for RemoteFwd {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        self.hostkey.check_server_key(key).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let host = self.local_host.clone();
        let port = self.local_port;
        // 0.62: 채널 수락이 명시적이 됐다 — accept 없이 드롭되면 자동 거절된다.
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut tcp) = TcpStream::connect((host.as_str(), port)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
            }
        });
        Ok(())
    }
}

/// 원격 포워딩(-R): 서버가 remote_port에서 받은 연결을 (local_host:local_port)로 전달.
/// 반환된 핸들이 드롭되면 포워딩이 끝난다(호출자가 보관).
pub async fn start_remote_forward(
    params: SshParams,
    remote_port: u16,
    local_host: String,
    local_port: u16,
) -> Result<Handle<RemoteFwd>, String> {
    let config = std::sync::Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let handler = RemoteFwd {
        local_host,
        local_port,
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
        _ => return Err("포워딩: 비밀번호 인증만 지원".into()),
    }

    handle
        .tcpip_forward("127.0.0.1", remote_port as u32)
        .await
        .map_err(|e| e.to_string())?;
    Ok(handle)
}
