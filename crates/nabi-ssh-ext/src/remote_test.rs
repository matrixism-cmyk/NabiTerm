//! 인-프로세스 SSH 서버로 원격(-R) 포워딩의 인바운드 경로를 런타임 검증한다.

use crate::start_remote_forward;
use nabi_proto::SshParams;
use russh::server::{self, Auth, Session};
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

/// 서버: -R 요청을 수락하고, 인바운드를 시뮬레이션해 forwarded-tcpip 채널로 "trigger" 전송.
struct RSrv;
impl server::Handler for RSrv {
    type Error = russh::Error;

    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let handle = session.handle();
        let addr = address.to_string();
        let p = *port;
        tokio::spawn(async move {
            if let Ok(channel) = handle
                .channel_open_forwarded_tcpip(addr, p, "127.0.0.1", 0)
                .await
            {
                let mut stream = channel.into_stream();
                let _ = stream.write_all(b"trigger").await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
        Ok(true)
    }
}

#[tokio::test]
async fn remote_forward_inbound_relay() {
    // -R 목적지(로컬 대상): 테스트가 제어하는 리스너.
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
    tokio::spawn(async move {
        if let Ok((mut conn, _)) = target.accept().await {
            let mut buf = vec![0u8; 7];
            let n = conn.read(&mut buf).await.unwrap_or(0);
            let _ = tx.send(buf[..n].to_vec());
        }
    });

    // SSH 서버.
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(rs) = server::run_stream(config, stream, RSrv).await {
                let _ = rs.await;
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(250)).await;
    let params = SshParams::password("127.0.0.1", addr.port(), "u", "p");
    let _handle = start_remote_forward(params, 9000, "127.0.0.1".to_string(), target_port)
        .await
        .expect("remote forward");

    let got = tokio::time::timeout(Duration::from_secs(3), rx)
        .await
        .expect("timeout")
        .expect("recv");
    assert_eq!(got, b"trigger", "인바운드 -R 릴레이 불일치");
}
