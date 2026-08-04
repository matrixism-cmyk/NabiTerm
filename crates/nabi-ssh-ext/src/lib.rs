//! nabi-ssh-ext — russh가 turn-key로 제공하지 않는 SSH 기능.
//!
//! 현재: 로컬 포트 포워딩(-L). 후속: 원격(-R)/동적(-D) SOCKS, ProxyJump, X11.
//! 로컬 포워딩은 인-프로세스 SSH 서버로 런타임 왕복 검증된다.

pub mod forward;
pub mod keygen;
pub mod proxy_jump;
pub mod remote;
pub mod socks5;
pub mod x11;

pub use forward::start_local_forward;
pub use proxy_jump::{connect_via_jump, JumpedSession};
pub use remote::start_remote_forward;
pub use socks5::start_dynamic_forward;
pub use x11::request_x11_forward;

#[cfg(test)]
mod forward_test;
#[cfg(test)]
mod proxy_test;
#[cfg(test)]
mod remote_test;
#[cfg(test)]
mod socks_test;
#[cfg(test)]
mod x11_test;
