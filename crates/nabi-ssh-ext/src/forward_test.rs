//! 인-프로세스 SSH 서버(direct-tcpip 에코)로 로컬 포워딩을 런타임 검증한다.

use crate::start_local_forward;
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
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
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
async fn local_forward_echo_roundtrip() {
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
    let local_port = start_local_forward(params, "target.invalid".to_string(), 80)
        .await
        .expect("start forward");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut c = TcpStream::connect(("127.0.0.1", local_port)).await.unwrap();
    c.write_all(b"ping").await.unwrap();

    let mut buf = [0u8; 4];
    let n = tokio::time::timeout(Duration::from_secs(3), c.read(&mut buf))
        .await
        .expect("timeout")
        .expect("read");
    assert_eq!(&buf[..n], b"ping", "포워딩 에코 불일치");
}
