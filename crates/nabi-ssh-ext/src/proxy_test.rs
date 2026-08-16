//! 인-프로세스 jump+target SSH 서버로 ProxyJump 체이닝을 런타임 검증한다.

use crate::connect_via_jump;
use nabi_proto::SshParams;
use russh::server::{self, Auth, Msg, Session};
use russh::Channel;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

fn config() -> Arc<server::Config> {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    })
}

/// 타겟 서버: 인증 수락 + 세션 채널 수락.
struct Target;
impl server::Handler for Target {
    type Error = russh::Error;
    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }
    async fn channel_open_session(
        &mut self,
        _c: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _s: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }
}

/// 점프 서버: direct-tcpip 요청을 실제 타겟 주소로 TCP 릴레이.
struct Jump;
impl server::Handler for Jump {
    type Error = russh::Error;
    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    #[allow(clippy::too_many_arguments)]
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _orig: &str,
        _orig_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let host = host.to_string();
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut tcp) = TcpStream::connect((host.as_str(), port as u16)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
            }
        });
        Ok(())
    }
}

async fn spawn_server<H: server::Handler + Send + 'static>(handler_fn: impl Fn() -> H + Send + 'static) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let cfg = config();
            let h = handler_fn();
            tokio::spawn(async move {
                if let Ok(rs) = server::run_stream(cfg, stream, h).await {
                    let _ = rs.await;
                }
            });
        }
    });
    port
}

#[tokio::test]
async fn proxy_jump_reaches_target() {
    let target_port = spawn_server(|| Target).await;
    let jump_port = spawn_server(|| Jump).await;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let jump = SshParams::password("127.0.0.1", jump_port, "u", "p");
    let target = SshParams::password("127.0.0.1", target_port, "u", "p");

    let session = connect_via_jump(&jump, &target).await.expect("proxyjump 연결");
    // 타겟 세션이 살아있어 채널을 열 수 있으면 체인이 동작한 것.
    let _ch = session
        .target
        .channel_open_session()
        .await
        .expect("타겟 세션 채널");
}
