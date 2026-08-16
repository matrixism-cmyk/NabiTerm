//! ProxyJump: 점프 호스트를 통해 타겟에 SSH 연결.
//!
//! jump에 연결 → jump에서 target으로 direct-tcpip 채널 개설 → 그 채널 스트림 위에서
//! 두 번째 SSH 세션을 connect_stream으로 수립한다. jump 핸들은 함께 살려둔다.

use crate::forward::{connect_authed, Fwd};
use nabi_proto::{SshAuth, SshParams};
use russh::client::{self, AuthResult, Handle};
use std::sync::Arc;

/// ProxyJump 결과 세션. `_jump`이 드롭되면 터널이 끊기므로 함께 보관한다.
pub struct JumpedSession {
    _jump: Handle<Fwd>,
    pub target: Handle<Fwd>,
}

/// jump를 통해 target에 연결·인증한다.
pub async fn connect_via_jump(
    jump: &SshParams,
    target: &SshParams,
) -> Result<JumpedSession, String> {
    let jump_handle = connect_authed(jump).await?;
    let channel = jump_handle
        .channel_open_direct_tcpip(target.host.clone(), target.port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| e.to_string())?;
    let stream = channel.into_stream();

    let config = Arc::new(client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    // 경유지 너머의 목적지도 호스트키를 검증한다(터널 안이라고 신뢰하지 않는다).
    let target_hostkey = Fwd::new(
        target.host.clone(),
        target.port,
        nabi_config::StorageLayout::resolve().known_hosts,
        None,
    );
    let mut target_handle = client::connect_stream(config, stream, target_hostkey)
        .await
        .map_err(|e| e.to_string())?;

    match &target.auth {
        SshAuth::Password(pw) => {
            let r = target_handle
                .authenticate_password(&target.user, pw)
                .await
                .map_err(|e| e.to_string())?;
            if !matches!(r, AuthResult::Success) {
                return Err(nabi_i18n::trc("net.jump.authfail").into());
            }
        }
        _ => return Err(nabi_i18n::trc("net.jump.pwonly").into()),
    }

    Ok(JumpedSession {
        _jump: jump_handle,
        target: target_handle,
    })
}
