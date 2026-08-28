//! 로컬 포트 포워딩(-L) + 포워딩군 공용 연결 헬퍼.
//!
//! 터널(-L/-R/-D/X11/점프)도 SSH 터미널과 **동일한** 호스트키 검증을 받는다. 특히 동적
//! 포워딩(SOCKS5)은 브라우저 트래픽 전체가 지나가므로 검증 없는 수락은 곧 중간자 노출이었다.

use nabi_proto::{SshAuth, SshParams};
use nabi_ssh::handler::ClientHandler;
use nabi_ssh::legacy::ConnOpts;
use russh::client::{self, AuthResult};
use std::sync::Arc;
use tokio::net::TcpListener;

/// 포워딩군 공용 클라이언트 핸들러(= SSH 터미널과 동일한 known_hosts 검증).
pub type Fwd = ClientHandler;

/// SSH에 연결·인증해 Handle을 반환한다(포워딩군 공용).
///
/// 호스트키는 known_hosts로 검증한다(미지 호스트는 TOFU 학습, **키 변경은 거부**).
/// 인증은 비밀번호와 개인키 파일을 모두 지원한다.
pub(crate) async fn connect_authed(params: &SshParams) -> Result<client::Handle<Fwd>, String> {
    // 유휴 터널이 NAT/서버 타임아웃으로 조용히 끊기지 않게 keepalive를 건다.
    let opts = ConnOpts::default();
    let known_hosts = nabi_config::StorageLayout::resolve().known_hosts;
    // 옛 서버 대응은 터미널·SFTP와 같은 경로를 쓴다(legacy.rs) — 포워딩만 못 붙으면 안 된다.
    let (mut handle, _) = nabi_ssh::legacy::connect_compat(&opts, |cfg| {
        let h = ClientHandler::new(params.host.clone(), params.port, known_hosts.clone(), None);
        async move { client::connect(cfg, (params.host.as_str(), params.port), h).await }
    })
    .await
    .map_err(|e| e.to_string())?;

    let result = match &params.auth {
        SshAuth::Password(pw) => handle
            .authenticate_password(&params.user, pw)
            .await
            .map_err(|e| e.to_string())?,
        // 키 인증도 지원(과거엔 비밀번호만 가능해 키 사용자는 포워딩 자체를 못 썼다).
        SshAuth::KeyFile { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref())
                .map_err(|e| format!("{}: {e}", nabi_i18n::trc("net.key.load")))?;
            let with_hash = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None);
            handle
                .authenticate_publickey(&params.user, with_hash)
                .await
                .map_err(|e| e.to_string())?
        }
        // 에이전트 인증(키가 에이전트에만 있는 사용자도 터널을 쓸 수 있게).
        SshAuth::Agent => {
            nabi_ssh::agent::authenticate_agent(&mut handle, &params.user)
                .await
                .map_err(|e| format!("{}: {e}", nabi_i18n::trc("net.fwd.agent")))?;
            AuthResult::Success
        }
        SshAuth::None => return Err(nabi_i18n::trc("net.fwd.noauth").into()),
    };
    if !matches!(result, AuthResult::Success) {
        return Err(nabi_i18n::trc("net.auth.fail").into());
    }
    Ok(handle)
}

/// 로컬 포트 포워딩(-L): 임시 로컬 포트의 각 연결을 (remote_host:remote_port)로 포워딩.
///
/// **핸들도 함께 돌려준다**(배치 AH). 포트만 주면 부르는 쪽이 "이 터널이 아직 살아 있나"를
/// 물어볼 방법이 없어, 연결이 끊겨도 화면은 계속 "활성"이라고 말한다. 핸들은 어차피
/// `Arc` 라 복제가 싸다.
pub async fn start_local_forward(
    params: SshParams,
    remote_host: String,
    remote_port: u16,
) -> Result<(u16, Arc<client::Handle<Fwd>>), String> {
    let handle = Arc::new(connect_authed(&params).await?);
    let watch = handle.clone();
    let listener = TcpListener::bind(("127.0.0.1", 0u16))
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let h = handle.clone();
            let rh = remote_host.clone();
            tokio::spawn(async move {
                if let Ok(channel) = h
                    .channel_open_direct_tcpip(rh, remote_port as u32, "127.0.0.1", 0)
                    .await
                {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                }
            });
        }
    });
    Ok((port, watch))
}
