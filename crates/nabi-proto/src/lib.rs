//! nabi-proto — UI↔오케스트레이터 버스 메시지 타입.
//!
//! 채널(전송자/수신자)이 아니라 메시지 **타입**만 정의한다. 동기(crossbeam)와
//! 비동기(tokio) 양쪽이 같은 어휘를 공유하도록 분리했다.

pub mod appctl;
pub mod command;
pub mod event;
pub mod pane_msg;
pub mod sftp;
pub mod shell;
pub mod shquote;
pub mod ssh;
pub mod stats;
pub mod trzsz;
pub mod statsparse;

pub use appctl::{AppCtl, SftpCtlOp};
pub use command::Command;
pub use event::Event;
pub use pane_msg::CommandBlock;
pub use sftp::{SftpEntry, SftpId};
pub use shell::ShellKind;
pub use ssh::{SshAuth, SshParams};
pub use stats::ServerStats;
pub use trzsz::{XferDecision, XferMode, XferProgress};
