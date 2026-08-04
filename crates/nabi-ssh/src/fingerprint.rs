//! 호스트키 지문(사용자 확인/TOFU 프롬프트용).

use russh::keys::{HashAlg, PublicKey};

/// 공개키의 SHA256 지문 문자열("SHA256:<base64>" 형식).
///
/// known_hosts 미지/변경 시 사용자에게 보여줄 값(후속 TOFU 다이얼로그에서 사용).
pub fn sha256_fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ssh-key 테스트 픽스처의 유효한 ed25519 공개키.
    const KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";

    #[test]
    fn fingerprint_is_sha256_prefixed_and_stable() {
        let key = PublicKey::from_openssh(KEY).expect("valid key");
        let fp = sha256_fingerprint(&key);
        assert!(fp.starts_with("SHA256:"), "got {fp}");
        assert_eq!(fp, sha256_fingerprint(&key));
    }
}
