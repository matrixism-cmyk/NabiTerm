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
    start_server_mode(false).await
}

/// 확장을 하나도 광고하지 않는 서버(옛 OpenSSH = 순정 SFTP v3 흉내).
async fn start_bare_server() -> std::net::SocketAddr {
    start_server_mode(true).await
}

async fn start_server_mode(bare: bool) -> std::net::SocketAddr {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let h = if bare { SshSession::bare() } else { SshSession::default() };
            if let Ok(rs) = server::run_stream(config, stream, h).await {
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

/// 확장 없는(옛) 서버에 접속해 SftpFs를 돌려준다 — v3 순정 경로 검증용.
pub(crate) async fn connect_bare_fs() -> crate::SftpFs {
    let addr = start_bare_server().await;
    let params = SshParams::password(addr.ip().to_string(), addr.port(), "u", "p");
    connect_sftp(&params, test_known_hosts(), None).await.expect("sftp connect")
}

/// 테스트용 임시 경로(매 호출 고유) — 이름이 겹치면 앞 테스트가 남긴 파일 때문에
/// "만들어지지 않아야 할 파일이 있다"는 식으로 엉뚱하게 실패한다(실제로 겪었다).
/// 실패한 테스트는 정리 코드를 건너뛰므로, 이름을 나누는 쪽이 확실하다.
pub(crate) fn tmp_path(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "nabi-{tag}-{}-{}",
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
