//! ssh-agent **포워딩** — 원격에서도 내 키로 서명할 수 있게 한다.
//!
//! 로컬 인증(`agent.rs`)은 "내가 이 서버에 로그인"하는 것이고, 포워딩은 "그 서버에서 다시
//! 다른 곳에 로그인"하는 것이다. 원격에서 `git push`나 다시 `ssh`를 쓰는 흐름에 필요하다.
//!
//! ## 어떻게 동작하는가
//!
//! 1. 세션 채널에 `auth-agent-req@openssh.com`을 보낸다(russh의 `Channel::agent_forward`).
//! 2. 서버가 서명이 필요할 때마다 `auth-agent@openssh.com` 채널을 **우리 쪽으로** 연다.
//! 3. 그 채널과 로컬 에이전트 사이를 바이트 그대로 이어 준다 — 그게 이 모듈이다.
//!
//! russh는 2번까지만 해 준다(기본 구현이 채널을 수락만 하고 아무것도 하지 않아, 원격이
//! 답을 기다리며 멈춘다). 3번을 우리가 붙여야 실제로 쓸 수 있다.
//!
//! ## 왜 바이트를 그대로 흘리는가
//!
//! 에이전트 프로토콜은 길이 앞머리가 붙은 메시지 열이다. 우리가 해석할 이유가 없다 —
//! 해석하면 새 메시지 종류가 생길 때마다 우리가 막는 셈이 된다. 그대로 흘리는 편이 맞고,
//! **개인키는 여전히 에이전트 밖으로 나오지 않는다**(서명 요청만 오간다).
//!
//! ## 보안상 무엇을 뜻하는가
//!
//! 포워딩을 켜면 **그 서버의 root는 세션이 살아 있는 동안 내 키로 서명을 시킬 수 있다.**
//! 키 자체는 못 가져가지만 그 시간 동안은 나인 척할 수 있다. 그래서 전역이 아니라
//! **세션마다** 켠다 — 믿는 서버에만.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

/// Windows OpenSSH ssh-agent의 기본 파이프 이름.
const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

/// 한 번에 옮기는 크기. 에이전트 메시지는 작다(서명 요청·응답 수 KB).
const BUF: usize = 8 * 1024;

/// 이 PC에서 포워딩에 쓸 수 있는 에이전트가 있는가.
///
/// Pageant는 창 메시지로 말하는 방식이라 바이트 통로가 없다 — 로컬 인증에는 쓰지만
/// 포워딩에는 쓸 수 없다. 켜 두고 조용히 안 되는 것보다 미리 아는 편이 낫다.
pub fn available() -> bool {
    ClientOptions::new().open(OPENSSH_PIPE).is_ok()
}

/// 서버가 연 에이전트 채널 하나를 로컬 에이전트에 이어 준다.
///
/// 한 채널 = 한 요청-응답 묶음이다. 어느 한쪽이 닫히면 둘 다 닫는다.
pub async fn serve_channel<R, W>(mut chan_rx: R, mut chan_tx: W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let pipe = ClientOptions::new().open(OPENSSH_PIPE)?;
    let (mut pr, mut pw) = tokio::io::split(pipe);
    let (mut a, mut b) = ([0u8; BUF], [0u8; BUF]);
    loop {
        tokio::select! {
            n = chan_rx.read(&mut a) => match n {
                Ok(0) | Err(_) => break,
                Ok(n) => pw.write_all(&a[..n]).await?,
            },
            n = pr.read(&mut b) => match n {
                Ok(0) | Err(_) => break,
                Ok(n) => chan_tx.write_all(&b[..n]).await?,
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 에이전트가 있든 없든 물어보는 것만으로 터지지 않는다.
    ///
    /// **런타임 안에서 묻는다.** 예전에는 그냥 `#[test]` 였는데, 그러면 이 PC 에
    /// ssh-agent 가 **켜져 있을 때만** 터졌다 — 파이프가 있어야 tokio 가 리액터를 찾고,
    /// 리액터가 없으면 그 자리에서 패닉한다("there is no reactor running").
    ///
    /// 그래서 이 시험은 **윈도우 서비스 상태에 따라 통과하다 말다 했다.** 2026-08-31에
    /// 실제로 그렇게 됐다 — 같은 코드가 아침에는 통과하고 저녁에는 깨졌다.
    /// 실제 호출은 늘 런타임 안에서 일어나므로, 시험도 그 조건을 맞춰야 한다.
    #[tokio::test]
    async fn asking_about_the_agent_is_safe() {
        let _ = available();
    }

    /// 파이프가 없으면 프록시는 **바로** 실패해야 한다 — 원격을 매달아 두면 안 된다.
    #[tokio::test]
    async fn without_an_agent_the_proxy_fails_fast() {
        if available() {
            return; // 이 머신에 에이전트가 켜져 있으면 이 시험은 뜻이 없다.
        }
        let (rx, tx) = (tokio::io::empty(), tokio::io::sink());
        assert!(serve_channel(rx, tx).await.is_err());
    }
}
