//! ssh-agent **포워딩** 실서버 검증 — 인프로세스로는 흉내 낼 수 없는 부분.
//!
//! ## 무엇을 보아야 하는가 (2026-08-25에 한 번 속았다)
//!
//! 처음에는 원격에서 `ssh-add -l`을 돌려 우리 키가 보이면 성공이라고 봤다. **틀렸다.**
//! 127.0.0.1로 자기 자신에게 붙으면 원격 셸이 *로컬* 에이전트를 그냥 본다 — 포워딩을
//! 껐을 때도 똑같이 보인다. 시험이 통과했지만 아무것도 증명하지 못했다.
//!
//! 진짜 판별식은 **원격의 `SSH_AUTH_SOCK`**이다. 포워딩이 되면 서버가 그 세션만의 소켓을
//! 만들어 이 변수에 넣어 준다. 안 되면 아예 없다. 그래서:
//!
//! * 대상은 **다른 기계**여야 한다(루프백이면 이 시험은 스스로를 거부한다).
//! * 대상은 **리눅스/유닉스 sshd**여야 한다 — Windows OpenSSH 서버는 에이전트 포워딩을
//!   구현하지 않는다(이 PC에서 `ssh -A`로 붙어도 `SSH_AUTH_SOCK`이 안 생기는 것을 확인했다).
//!
//! ## 준비(스킬 §4 절차대로, 끝나면 원상복구)
//!
//! ```text
//! Set-Service ssh-agent -StartupType Manual; Start-Service ssh-agent
//! ssh-keygen … ; ssh-add … ; 대상 리눅스의 ~/.ssh/authorized_keys 에 공개키 추가
//! $env:NABI_RT_HOST="<리눅스 호스트>"; $env:NABI_RT_USER="<계정>"; $env:NABI_RT_AGENT_FWD="1"
//! cargo test -p nabi-ssh agent_forward -- --ignored --nocapture
//! ```

use crate::{connect, SshParams};
use bytes::Bytes;
use crossbeam_channel::unbounded;
use nabi_pty::ByteChannel;
use nabi_types::{GridSize, PaneId};
use std::time::Duration;

/// 포워딩을 켜면 원격에 **세션 전용 에이전트 소켓**이 생긴다 — 유일한 확실한 증거.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "실 리눅스 서버 + ssh-agent 필요(NABI_RT_HOST/USER/AGENT_FWD)"]
async fn agent_forwarding_gives_the_remote_its_own_socket() {
    let Some((host, user)) = target() else { return };
    let out = run_remote(&host, &user, true, "echo SOCK=[$SSH_AUTH_SOCK]").await;
    assert!(
        out.contains("SOCK=[/") || out.contains("SOCK=[\\"),
        "포워딩을 켰는데 원격에 에이전트 소켓이 없다:
{out}"
    );
}

/// **끈 상태에서는 소켓이 없어야 한다.** 이걸 같이 보지 않으면 "원래 되던 것"을 성공으로
/// 착각한다 — 실제로 그렇게 한 번 속았다(모듈 주석 참고).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "실 리눅스 서버 + ssh-agent 필요(NABI_RT_HOST/USER/AGENT_FWD)"]
async fn without_forwarding_the_remote_has_no_socket() {
    let Some((host, user)) = target() else { return };
    let out = run_remote(&host, &user, false, "echo SOCK=[$SSH_AUTH_SOCK]").await;
    assert!(out.contains("SOCK=[]"), "포워딩을 껐는데 원격에 소켓이 있다:
{out}");
}

/// 검증 대상. 준비가 안 됐거나 **루프백이면 건너뛴다** — 자기 자신은 판별력이 없다.
fn target() -> Option<(String, String)> {
    if std::env::var("NABI_RT_AGENT_FWD").is_err() {
        eprintln!("NABI_RT_AGENT_FWD 없음 — 건너뜀");
        return None;
    }
    let host = std::env::var("NABI_RT_HOST").ok()?;
    let user = std::env::var("NABI_RT_USER").ok()?;
    if matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1") {
        eprintln!("루프백은 포워딩 여부를 가릴 수 없다 — 건너뜀");
        return None;
    }
    Some((host, user))
}

/// 실제 서버에 붙어 명령 한 줄을 치고 화면을 모아 돌려준다.
async fn run_remote(host: &str, user: &str, forward: bool, cmd: &str) -> String {
    let (out_tx, out_rx) = unbounded::<(PaneId, Bytes)>();
    let params = SshParams {
        host: host.into(),
        port: 22,
        user: user.into(),
        auth: nabi_proto::SshAuth::Agent,
        jump: None,
        agent_forward: forward,
        env: Vec::new(),
    };
    let kh = std::env::temp_dir().join(format!("nabi-fwd-known-{}", std::process::id()));
    let _ = std::fs::remove_file(&kh);
    let mut ch = connect(
        &tokio::runtime::Handle::current(),
        PaneId::new(1),
        params,
        GridSize::new(100, 30),
        out_tx,
        kh.clone(),
        None,             // 호스트키 자동 학습(시험).
        Box::new(|_| {}), // 종료 통지 무시.
        None,             // 통계 폴링 안 함.
    );
    // 셸이 뜰 시간을 준 뒤 명령 한 줄.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let _ = ch.write(format!("{cmd}\r").as_bytes());
    tokio::time::sleep(Duration::from_secs(4)).await;
    let _ = std::fs::remove_file(&kh);
    let mut text = String::new();
    while let Ok((_, b)) = out_rx.try_recv() {
        text.push_str(&String::from_utf8_lossy(&b));
    }
    text
}
