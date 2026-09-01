//! nabi-ftp — FTP/FTPS 백엔드(suppaftp). `nabi_fs::RemoteFs` 구현.
//!
//! ⚠️ 구조 구현(컴파일 검증). 런타임 동작 확인에는 FTP 서버가 필요하다.
//! ⚠️ 평문 FTP는 비밀번호를 평문 전송하므로 FTPS(tokio-rustls-ring) 사용을 권장한다(후속).

pub mod fs;
pub mod ftpsafe;

pub use fs::{connect_ftp, FtpFs};

#[cfg(test)]
mod ftp_test;
