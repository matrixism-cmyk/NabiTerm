//! 인-프로세스 russh 에코 서버로 SSH 클라이언트를 런타임 검증한다.
//!
//! 외부 SSH 서버 없이도 연결·인증·PTY·셸·입출력 왕복을 실제로 확인한다.

use crate::{connect, SshParams};
use bytes::Bytes;
use crossbeam_channel::unbounded;
use nabi_pty::ByteChannel;
use nabi_types::{GridSize, PaneId};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, Pty};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

struct Echo;

impl server::Handler for Echo {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        _term: &str,
        _col: u32,
        _row: u32,
        _pw: u32,
        _ph: u32,
        _modes: &[(Pty, u32)],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn shell_request(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, Bytes::copy_from_slice(data))?;
        Ok(())
    }
}

// 고정 테스트 호스트키(ssh-key 픽스처). rng 버전 의존을 피하려고 임베드한다.
const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

#[tokio::test]
async fn ssh_client_echo_roundtrip() {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(running) = server::run_stream(config, stream, Echo).await {
                let _ = running.await;
            }
        }
    });

    let (out_tx, out_rx) = unbounded::<(PaneId, Bytes)>();
    let known_hosts = std::env::temp_dir().join(format!("nabi-kh-{}", std::process::id()));
    let params = SshParams::password(addr.ip().to_string(), addr.port(), "user", "pw");
    let mut ch = connect(
        &tokio::runtime::Handle::current(),
        PaneId::new(1),
        params,
        GridSize::new(80, 24),
        out_tx,
        known_hosts.clone(),
        None, // 테스트: 호스트키 자동 학습(TOFU).
        Box::new(|_| {}), // 테스트: 종료 통지 무시.
        None,             // 테스트: 서버 통계 폴링 안 함.
    );

    tokio::time::sleep(Duration::from_millis(500)).await;
    ByteChannel::write(&mut ch, b"hello\n").unwrap();

    let mut got = Vec::new();
    for _ in 0..50 {
        while let Ok((_, b)) = out_rx.try_recv() {
            got.extend_from_slice(&b);
        }
        if got.windows(5).any(|w| w == b"hello") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = std::fs::remove_file(&known_hosts);

    assert!(
        got.windows(5).any(|w| w == b"hello"),
        "에코 미수신: {:?}",
        String::from_utf8_lossy(&got)
    );
}
