//! nabi-pty — ConPTY 로컬 셸 전송 계층(portable-pty).
//!
//! `ByteChannel` 트레잇으로 로컬 PTY와 (후속) SSH 채널을 동일한 바이트
//! 소스/싱크로 추상화한다 → VT 모델이 로컬/원격에 동일하게 동작.

pub mod envclean;
mod channel_trait;
pub mod conpty;
pub mod pump;
pub mod spawn;

pub use channel_trait::ByteChannel;
pub use conpty::{spawn_local, LocalPty};
pub use pump::OUT_BUS_CAPACITY;
pub use spawn::{build_command, is_store_alias, program_name, resolve_program, resolve_shell, wsl_distros};
