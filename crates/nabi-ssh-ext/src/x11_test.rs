//! 인-프로세스 SSH 서버로 X11 포워딩의 인바운드 경로를 런타임 검증한다.
//! 더미 로컬 X 엔드포인트(리스너)로 x11 채널 데이터가 전달되는지 확인.

use crate::request_x11_forward;
use nabi_proto::SshParams;
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

struct XSrv;

impl server::Handler for XSrv {
    type Error = russh::Error;

    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn x11_request(
        &mut self,
        _channel: ChannelId,
        _single_connection: bool,
        _x11_protocol: &str,
        _x11_cookie: &str,
        _x11_screen: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let handle = session.handle();
        tokio::spawn(async move {
            if let Ok(channel) = handle.channel_open_x11("127.0.0.1", 6000).await {
                let mut stream = channel.into_stream();
                let _ = stream.write_all(b"Xdata").await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
        Ok(())
    }
}

#[tokio::test]
async fn x11_forward_inbound_relay() {
    // 더미 로컬 X 서버.
    let xserver = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let x_port = xserver.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
    tokio::spawn(async move {
        if let Ok((mut conn, _)) = xserver.accept().await {
            let mut buf = vec![0u8; 5];
            let n = conn.read(&mut buf).await.unwrap_or(0);
            let _ = tx.send(buf[..n].to_vec());
        }
    });

    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(rs) = server::run_stream(config, stream, XSrv).await {
                let _ = rs.await;
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(250)).await;
    let params = SshParams::password("127.0.0.1", addr.port(), "u", "p");
    let _handle = request_x11_forward(params, "127.0.0.1".to_string(), x_port)
        .await
        .expect("x11 forward");

    let got = tokio::time::timeout(Duration::from_secs(3), rx)
        .await
        .expect("timeout")
        .expect("recv");
    assert_eq!(got, b"Xdata", "X11 인바운드 릴레이 불일치");
}
