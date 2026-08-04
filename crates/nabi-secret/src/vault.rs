//! 자격증명 볼트(암호화 봉투: salt ++ nonce ++ ciphertext).

use crate::aead::{open, seal};
use crate::error::VaultError;
use crate::kdf::derive_key;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// 비밀 저장소(키→비밀). 평문은 메모리에만 존재한다.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vault {
    pub entries: BTreeMap<String, String>,
}

impl Vault {
    pub fn set(&mut self, key: impl Into<String>, secret: impl Into<String>) {
        self.entries.insert(key.into(), secret.into());
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }
}

/// 볼트를 master password로 암호화한다: salt(16) ++ nonce(12) ++ ciphertext.
pub fn seal_vault(vault: &Vault, password: &str) -> Result<Vec<u8>, VaultError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);

    let key = derive_key(password.as_bytes(), &salt)?;
    let plaintext = serde_json::to_vec(vault).map_err(|e| VaultError::Serde(e.to_string()))?;
    let ct = seal(&key, &nonce, &plaintext)?;

    let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ct.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 암호화된 볼트를 master password로 복호화한다(잘못된 비밀번호 → Crypto 오류).
pub fn open_vault(bytes: &[u8], password: &str) -> Result<Vault, VaultError> {
    if bytes.len() < SALT_LEN + NONCE_LEN {
        return Err(VaultError::Format);
    }
    let salt = &bytes[..SALT_LEN];
    let nonce: [u8; NONCE_LEN] = bytes[SALT_LEN..SALT_LEN + NONCE_LEN]
        .try_into()
        .map_err(|_| VaultError::Format)?;
    let ct = &bytes[SALT_LEN + NONCE_LEN..];

    let key = derive_key(password.as_bytes(), salt)?;
    let plaintext = open(&key, &nonce, ct)?;
    serde_json::from_slice(&plaintext).map_err(|e| VaultError::Serde(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_wrong_password() {
        let mut v = Vault::default();
        v.set("host:prod", "s3cr3t-pw");

        let sealed = seal_vault(&v, "master-pw").unwrap();
        // 평문 비밀이 암호문에 노출되지 않는다.
        assert!(!sealed.windows(9).any(|w| w == b"s3cr3t-pw"));

        let opened = open_vault(&sealed, "master-pw").unwrap();
        assert_eq!(opened.get("host:prod"), Some("s3cr3t-pw"));

        // 잘못된 비밀번호 → 복호화 실패(태그 검증).
        assert!(open_vault(&sealed, "wrong-pw").is_err());
    }

    #[test]
    fn rejects_truncated_vault() {
        assert!(open_vault(&[0u8; 4], "x").is_err());
    }
}
