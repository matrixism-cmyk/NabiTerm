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

/// 한 연결에서 시도할 최대 키 수. 키 하나가 곧 인증 시도 한 번이고, 서버의
/// `MaxAuthTries`(OpenSSH 기본 6)를 넘기면 **서버가 연결을 끊는다**. 에이전트에 키를
/// 여러 개 올려 둔 사람이 많아서, 상한 없이 돌면 맞는 키에 닿기 전에 끊길 수 있다.
const MAX_KEYS: usize = 6;

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
    let mut tried = 0usize;
    if let Ok(a) = AgentClient::connect_named_pipe(OPENSSH_PIPE).await {
        match try_identities(handle, user, a).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(n)) => tried = tried.max(n),
            Err(()) => {}
        }
    }
    if let Ok(a) = AgentClient::connect_pageant().await {
        match try_identities(handle, user, a).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(n)) => tried = tried.max(n),
            Err(()) => {}
        }
    }
    Err(match tried {
        0 => "쓸 수 있는 ssh-agent가 없습니다(에이전트 미실행 또는 등록된 키 없음)".to_string(),
        // 상한까지 갔다면 뒤에 남은 키를 못 써 본 것이다 — 그 사실을 알려 준다.
        n if n >= MAX_KEYS => format!(
            "에이전트 키 {n}개가 모두 거부됐습니다(서버 MaxAuthTries 때문에 더 시도하지 않습니다). 쓰지 않는 키를 에이전트에서 빼거나 키 파일을 직접 지정하세요"
        ),
        n => format!("에이전트의 키 {n}개를 서버가 모두 거부했습니다"),
    })
}

/// 에이전트가 가진 공개키를 순서대로 시도한다. 서명은 에이전트가 한다.
///
/// `Ok(Ok(()))`=인증 성공, `Ok(Err(n))`=키 n개를 써 봤지만 전부 거부,
/// `Err(())`=이 에이전트는 못 쓴다(연결은 됐지만 키 목록을 못 받았거나 비어 있다).
#[allow(clippy::result_unit_err)]
async fn try_identities<H, R>(
    handle: &mut Handle<H>,
    user: &str,
    mut agent: AgentClient<R>,
) -> Result<Result<(), usize>, ()>
where
    H: russh::client::Handler,
    R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let ids = agent.request_identities().await.map_err(|_| ())?;
    if ids.is_empty() {
        return Err(());
    }
    let mut used = 0usize;
    for id in ids.iter().take(MAX_KEYS) {
        // 인증서(OpenSSH certificate)는 별도 경로라 여기서는 평범한 공개키만 쓴다.
        let AgentIdentity::PublicKey { key, .. } = id else { continue };
        let key = key.clone();
        used += 1;
        // 실패는 그 키가 서버에 없다는 뜻일 뿐이므로 다음 키로 넘어간다.
        if let Ok(AuthResult::Success) =
            handle.authenticate_publickey_with(user, key, None, &mut agent).await
        {
            return Ok(Ok(()));
        }
    }
    if used > 0 {
        Ok(Err(used))
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
