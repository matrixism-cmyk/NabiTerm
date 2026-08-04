//! Argon2id 키 파생.

use crate::error::VaultError;
use argon2::Argon2;
use zeroize::Zeroizing;

/// 마스터 비밀번호 + salt → 32바이트 키(Argon2id, 메모리 하드).
///
/// salt는 8바이트 이상이어야 한다. 반환 키는 drop 시 zeroize 된다.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::default()
        .hash_password_into(password, salt, key.as_mut())
        .map_err(|_| VaultError::Kdf)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::derive_key;

    #[test]
    fn deterministic_and_salt_password_sensitive() {
        let salt = b"saltsalt"; // 8바이트.
        let a = derive_key(b"pw", salt).unwrap();
        assert_eq!(*a, *derive_key(b"pw", salt).unwrap()); // 같은 입력 → 같은 키(볼트 해제 가능 근거).
        assert_ne!(*a, *derive_key(b"pw", b"different").unwrap()); // salt 다르면 키 다름.
        assert_ne!(*a, *derive_key(b"other", salt).unwrap()); // 비밀번호 다르면 키 다름.
    }

    #[test]
    fn short_salt_errors() {
        assert!(derive_key(b"pw", b"short").is_err()); // salt < 8바이트 → Kdf 오류.
    }
}
