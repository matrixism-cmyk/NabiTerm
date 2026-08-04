//! AES-256-GCM 봉인/해제.

use crate::error::VaultError;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

/// 평문을 봉인한다(인증 태그 포함).
pub fn seal(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|_| VaultError::Crypto)
}

/// 봉인을 해제한다(태그 검증 실패=잘못된 비밀번호/손상 → Crypto 오류).
pub fn open(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| VaultError::Crypto)
}

#[cfg(test)]
mod tests {
    use super::{open, seal};

    #[test]
    fn seal_open_roundtrip() {
        let (key, nonce) = ([7u8; 32], [3u8; 12]);
        let pt: &[u8] = b"secret password";
        let ct = seal(&key, &nonce, pt).unwrap();
        assert_ne!(ct.as_slice(), pt); // 암호문 ≠ 평문.
        assert_eq!(open(&key, &nonce, &ct).unwrap().as_slice(), pt); // 라운드트립 복원.
    }

    #[test]
    fn wrong_key_or_tamper_fails() {
        let (key, nonce) = ([7u8; 32], [3u8; 12]);
        let ct = seal(&key, &nonce, b"data").unwrap();
        assert!(open(&[8u8; 32], &nonce, &ct).is_err()); // 잘못된 키.
        assert!(open(&key, &[4u8; 12], &ct).is_err()); // 잘못된 nonce.
        let mut bad = ct.clone();
        bad[0] ^= 1;
        assert!(open(&key, &nonce, &bad).is_err()); // 변조 → 태그 검증 실패.
    }
}
