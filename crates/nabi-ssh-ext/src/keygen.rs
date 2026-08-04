//! SSH 키페어 생성(PuTTYgen/ssh-keygen 상당). ed25519 키를 OpenSSH 형식으로 만든다.
//! OS 난수 32바이트로 시드를 만들어 from_seed로 구성 — ssh-key의 rand_core 버전과 무관(버전 충돌 회피).

use rand::RngCore;
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::keys::ssh_key::LineEnding;
use russh::keys::PrivateKey;

/// 새 ed25519 SSH 키페어를 만든다 → (OpenSSH 개인키 PEM, "ssh-ed25519 … comment" 공개키 한 줄, SHA256 지문).
pub fn generate_ed25519(comment: &str) -> Result<(String, String, String), String> {
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed); // OS CSPRNG(rand 0.8) — rng trait 버전 충돌 회피.
    let kp = Ed25519Keypair::from_seed(&seed);
    let key = PrivateKey::new(KeypairData::Ed25519(kp), comment).map_err(|e| e.to_string())?;
    let priv_pem = key.to_openssh(LineEnding::LF).map_err(|e| e.to_string())?.to_string();
    let pub_line = key.public_key().to_openssh().map_err(|e| e.to_string())?;
    let fp = key.public_key().fingerprint(russh::keys::HashAlg::Sha256).to_string(); // 서버 키 대조용 지문.
    Ok((priv_pem, pub_line, fp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_ed25519() {
        let (priv_pem, pub_line, fp) = generate_ed25519("test@nabi").unwrap();
        assert!(PrivateKey::from_openssh(&priv_pem).is_ok()); // 재파싱 가능 = 유효한 키.
        assert!(pub_line.starts_with("ssh-ed25519 "));
        assert!(pub_line.trim_end().ends_with("test@nabi"));
        assert!(fp.starts_with("SHA256:")); // 표준 지문 표기.
        assert_ne!(generate_ed25519("").unwrap().0, priv_pem); // 매번 다른 키(난수성).
    }
}
