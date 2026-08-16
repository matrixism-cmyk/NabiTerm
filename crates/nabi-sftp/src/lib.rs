//! nabi-sftp — SFTP 백엔드(russh-sftp). `nabi_fs::RemoteFs` 구현.
//!
//! ⚠️ 구조 구현(컴파일 검증). 런타임 동작 확인에는 실제 SSH 서버가 필요하다.
//! MobaXterm식으로 기존 SSH 연결을 재사용하려면 오케스트레이터에서 핸들을 공유하도록
//! 확장한다(현재는 별도 연결 + 비밀번호 인증).

pub mod fs;
pub mod hashcheck;
mod pipeline;
pub mod raw;
mod recurse;
pub mod session;
mod xfer;

pub use fs::SftpFs;
pub use hashcheck::SFTP_VERIFY_HASH;
pub use recurse::DirProgress;
pub use session::connect_sftp;

#[cfg(test)]
mod sftp_server;
#[cfg(test)]
mod sftp_boot;
#[cfg(test)]
mod sftp_serverext;
#[cfg(test)]
mod sftp_test;
#[cfg(test)]
mod pipeline_test;
#[cfg(test)]
mod realserver_test;
#[cfg(test)]
mod realserver_pipe;
