//! nabi-secret — 자격증명 볼트.
//!
//! 마스터 비밀번호 → Argon2id KDF → AES-256-GCM으로 볼트를 암호화한다.
//! 비밀은 디스크에 평문으로 저장하지 않으며, 메모리에서는 zeroize로 와이프한다.
//!
//! ⚠️ 후속: keyring(Windows Credential Manager) 연동, 자동 잠금, Windows Hello.
//! ⚠️ 포터블/동기화 저장 시에는 DPAPI/Credential Manager를 쓰지 않고 이 master-password
//! 볼트만 사용한다(타 PC 복호 불가 방지) — [[sessions-storage]].

pub mod aead;
pub mod error;
pub mod kdf;
pub mod keyringstore;
pub mod secret_box;
pub mod vault;

pub use error::VaultError;
pub use secret_box::{SecretString, Zeroizing};
pub use vault::{open_vault, seal_vault, Vault};
