//! nabi-orchestrator — 제어 평면(액터).
//!
//! 모든 Pane 상태(ByteChannel 전송 + VT 모델 + OSC 탭)를 PaneId로 단일 소유한다.
//! UI는 Command를 보내고 Event를 받으며, 렌더용 모델은 공유 레지스트리로 읽는다.
//!
//! M1은 스레드 + crossbeam 채널로 구현한다(네트워크/SSH 도입 시 tokio seam 추가).

pub mod actor;
pub mod commands;
mod connsftp;
pub mod handle;
pub mod hostkey;
pub mod pane_meta;
pub mod pane_registry;
pub mod router;
pub mod sftp;
mod sftppool;
#[cfg(test)]
mod sftppool_test;
mod sftpretry;
pub mod spawn_pane;
pub mod trzszfile;
pub mod trzszpane;

pub use actor::start;
pub use handle::OrchestratorHandle;
pub use pane_registry::{PaneView, SharedModel, SharedPanes};
