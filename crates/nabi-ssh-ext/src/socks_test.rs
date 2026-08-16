//! 인-프로세스 SSH 서버로 동적(-D) SOCKS5 포워딩을 런타임 검증한다.

use crate::start_dynamic_forward;
use nabi_proto::SshParams;
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

struct Srv;

impl server::Handler for Srv {
    type Error = russh::Error;

    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    #[allow(clippy::too_many_arguments)]
    async fn channel_open_direct_tcpip(
        &mut self,
        _channel: Channel<Msg>,
        _host: &str,
        _port: u32,
        _orig: &str,
        _orig_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, bytes::Bytes::copy_from_slice(data))?;
        Ok(())
    }
}

#[tokio::test]
async fn dynamic_socks_forward_roundtrip() {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(rs) = server::run_stream(config, stream, Srv).await {
                let _ = rs.await;
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(250)).await;
    let params = SshParams::password(addr.ip().to_string(), addr.port(), "u", "p");
    let socks_port = start_dynamic_forward(params).await.expect("start socks");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut c = TcpStream::connect(("127.0.0.1", socks_port)).await.unwrap();
    // SOCKS5 greeting (no-auth)
    c.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut g = [0u8; 2];
    c.read_exact(&mut g).await.unwrap();
    assert_eq!(g, [0x05, 0x00]);
    // CONNECT echo.test:1234 (domain)
    let host = b"echo.test";
    let mut req = vec![0x05, 0x01, 0x00, 0x03, host.len() as u8];
    req.extend_from_slice(host);
    req.extend_from_slice(&1234u16.to_be_bytes());
    c.write_all(&req).await.unwrap();
    let mut rep = [0u8; 10];
    c.read_exact(&mut rep).await.unwrap();
    assert_eq!(rep[1], 0x00, "SOCKS connect 실패");

    // 릴레이: ping → 서버 에코 → ping
    c.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    let n = tokio::time::timeout(Duration::from_secs(3), c.read(&mut buf))
        .await
        .expect("timeout")
        .expect("read");
    assert_eq!(&buf[..n], b"ping", "SOCKS 포워딩 에코 불일치");
}
