//! nabi-ssh — SSH 원격 세션 전송(russh).
//!
//! 원격 pane은 ConPTY와 동일한 `ByteChannel`로 추상화되어 VT 모델이 로컬/원격에
//! 동일하게 동작한다. 입력은 tokio mpsc로 SSH 태스크에 전달하고, 출력은 오케스트레이터
//! 출력 버스(crossbeam)로 흘려보낸다(plan의 crossbeam↔tokio seam).
//!
//! ⚠️ M2 진행 중: 호스트키는 현재 TOFU 임시 수락(known_hosts 헬퍼·프롬프트는 후속),
//! 에이전트 인증·ProxyJump·포워딩은 nabi-ssh-ext에서. 런타임 검증은 SSH 서버 필요.

pub mod channel;
pub mod fingerprint;
pub mod keychange;
pub mod knownhosts;
pub mod agent;
pub mod agentfwd;
pub mod diagnose;
pub mod envvars;
#[cfg(test)]
mod agentfwd_test;
pub mod conntimeout;
pub mod copyid;
pub mod legacy;
pub mod handler;
pub mod kexinfo;
pub mod keygen;
pub mod params;
pub mod session;
pub mod stats;
pub mod verify;

#[cfg(test)]
mod echo_test;

pub use channel::SshChannel;
pub use fingerprint::sha256_fingerprint;
pub use knownhosts::{known_hosts_dedupe, known_hosts_find, known_hosts_match, known_hosts_remove, parse_known_hosts_line, KnownHostEntry};
pub use params::{SshAuth, SshParams};
pub use session::connect;
pub use verify::{HostKeyInfo, HostKeyVerifier, HostKeyVerify};
