//! 테스트 하네스 부팅 — 인프로세스 SSH 서버 기동 + 접속 헬퍼. 핸들러 구현은 sftp_server.rs.

use crate::connect_sftp;
use crate::sftp_server::{SshSession, SERVER_KEY};
use nabi_proto::SshParams;
use russh::server;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
/// 인프로세스 SSH+SFTP 서버를 띄우고 접속 주소를 돌려준다.
async fn start_server() -> std::net::SocketAddr {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(rs) = server::run_stream(config, stream, SshSession::default()).await {
                let _ = rs.await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(250)).await;
    addr
}

/// 테스트용 known_hosts 경로(매 호출 고유) — TOFU 학습이 실제 사용자 파일을 건드리지 않게.
pub(crate) fn test_known_hosts() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "nabi-test-known-hosts-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

/// 인프로세스 서버에 접속해 SftpFs를 돌려준다(테스트 진입점).
pub(crate) async fn connect_fs() -> crate::SftpFs {
    let addr = start_server().await;
    let params = SshParams::password(addr.ip().to_string(), addr.port(), "u", "p");
    // verifier 없음 → 미지 호스트는 TOFU 학습(임시 파일). 키 변경은 여전히 거부된다.
    connect_sftp(&params, test_known_hosts(), None).await.expect("sftp connect")
}
