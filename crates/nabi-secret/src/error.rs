//! 볼트 에러.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("키 파생(Argon2) 실패")]
    Kdf,
    #[error("암호화/복호화 실패(잘못된 비밀번호이거나 손상된 볼트)")]
    Crypto,
    #[error("볼트 형식 오류")]
    Format,
    #[error("직렬화 오류: {0}")]
    Serde(String),
}
