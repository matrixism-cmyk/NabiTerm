//! ssh-agent 인증 — 실행 중인 에이전트가 들고 있는 키로 로그인한다.
//!
//! 키 파일 경로를 지정하거나 암호를 매번 입력할 필요가 없다. Windows에는 에이전트가 둘 있어
//! 둘 다 본다: **OpenSSH ssh-agent**(네임드 파이프)와 **Pageant**(PuTTY 계열).
//!
//! 개인키는 에이전트 밖으로 나오지 않는다 — 서명만 부탁하고 결과를 받는다. 그래서
//! 우리 프로세스가 키를 손에 쥐지 않는다는 점에서 키 파일 방식보다 안전하다.

use russh::client::{AuthResult, Handle};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use tokio::io::{AsyncRead, AsyncWrite};

/// Windows OpenSSH ssh-agent의 기본 파이프 이름.
const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

/// 에이전트에 담긴 키들로 차례로 인증을 시도한다. 하나라도 통과하면 `Ok(())`.
///
/// OpenSSH 에이전트를 먼저 보고, 없으면 Pageant를 본다. 둘 다 없거나 키가 하나도
/// 받아들여지지 않으면 실패 사유를 문자열로 돌려준다(호출 쪽이 사용자에게 보여 준다).
pub async fn authenticate_agent<H: russh::client::Handler>(
    handle: &mut Handle<H>,
    user: &str,
) -> Result<(), String> {
    // 열려 있는 파이프가 곧 쓸 수 있는 에이전트라는 뜻은 아니다 — 서비스를 내려도 연결
    // 자체는 되고 목록 요청에서 "early eof"가 났다. 그래서 **키를 실제로 받아온 것**만
    // 시도로 친다. 앞 에이전트가 답을 못 줘도 다음 에이전트를 계속 본다.
    let mut had_keys = false;
    if let Ok(a) = AgentClient::connect_named_pipe(OPENSSH_PIPE).await {
        match try_identities(handle, user, a).await {
            Ok(true) => return Ok(()),
            Ok(false) => had_keys = true,
            Err(()) => {}
        }
    }
    if let Ok(a) = AgentClient::connect_pageant().await {
        match try_identities(handle, user, a).await {
            Ok(true) => return Ok(()),
            Ok(false) => had_keys = true,
            Err(()) => {}
        }
    }
    Err(match had_keys {
        true => "에이전트의 키를 서버가 모두 거부했습니다".to_string(),
        false => "쓸 수 있는 ssh-agent가 없습니다(에이전트 미실행 또는 등록된 키 없음)".to_string(),
    })
}

/// 에이전트가 가진 공개키를 순서대로 시도한다. 서명은 에이전트가 한다.
///
/// `Ok(true)`=인증 성공, `Ok(false)`=키는 있었지만 전부 거부, `Err(())`=이 에이전트는 못 쓴다.
async fn try_identities<H, R>(
    handle: &mut Handle<H>,
    user: &str,
    mut agent: AgentClient<R>,
) -> Result<bool, ()>
where
    H: russh::client::Handler,
    R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let ids = agent.request_identities().await.map_err(|_| ())?;
    if ids.is_empty() {
        return Err(());
    }
    let mut usable = false;
    for id in ids {
        // 인증서(OpenSSH certificate)는 별도 경로라 여기서는 평범한 공개키만 쓴다.
        let AgentIdentity::PublicKey { key, .. } = &id else { continue };
        let key = key.clone();
        usable = true;
        // 실패는 그 키가 서버에 없다는 뜻일 뿐이므로 다음 키로 넘어간다.
        if let Ok(AuthResult::Success) =
            handle.authenticate_publickey_with(user, key, None, &mut agent).await
        {
            return Ok(true);
        }
    }
    if usable {
        Ok(false)
    } else {
        Err(())
    }
}

/// 에이전트에 들어 있는 키 설명 목록(설정 화면에서 "지금 뭐가 올라와 있나" 보여줄 때).
/// 에이전트가 없으면 빈 목록.
pub async fn agent_identities() -> Vec<String> {
    async fn list<R>(mut a: AgentClient<R>) -> Vec<String>
    where
        R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        a.request_identities()
            .await
            .unwrap_or_default()
            .iter()
            .map(describe)
            .collect()
    }
    if let Ok(a) = AgentClient::connect_named_pipe(OPENSSH_PIPE).await {
        let v = list(a).await;
        if !v.is_empty() {
            return v;
        }
    }
    match AgentClient::connect_pageant().await {
        Ok(a) => list(a).await,
        Err(_) => Vec::new(),
    }
}

/// 키 한 개를 사람이 읽는 한 줄로(주석이 있으면 주석, 없으면 알고리즘 이름).
fn describe(id: &AgentIdentity) -> String {
    match id {
        AgentIdentity::PublicKey { key, comment } if !comment.is_empty() => {
            format!("{comment} ({})", key.algorithm())
        }
        AgentIdentity::PublicKey { key, .. } => key.algorithm().to_string(),
        _ => "certificate".to_string(),
    }
}
