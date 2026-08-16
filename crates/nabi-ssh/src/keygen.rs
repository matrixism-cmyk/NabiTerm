//! SSH 키 생성(T1 보안 편의) — ed25519 키쌍을 OpenSSH 형식으로 만든다(순수, 파일 저장은 호출측).
//!
//! MobaXterm/Termius의 키 생성기 상당. RSA는 의도적으로 제외 — 현대 기본값(ed25519)만
//! 제공해 잘못된 선택지를 없앤다(짧고, 빠르고, 모든 최신 서버가 지원).

use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};

/// ed25519 키쌍 생성 → (개인키 OpenSSH PEM, 공개키 한 줄). 코멘트는 공개키 식별용.
pub fn generate_ed25519(comment: &str) -> Result<(String, String), String> {
    let mut rng = rand::rng();
    let mut key = PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(|e| e.to_string())?;
    if !comment.trim().is_empty() {
        key.set_comment(comment.trim());
    }
    let pem = key.to_openssh(LineEnding::LF).map_err(|e| e.to_string())?.to_string();
    let pub_line = key.public_key().to_openssh().map_err(|e| e.to_string())?;
    Ok((pem, pub_line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_parseable_ed25519() {
        let (pem, pub_line) = generate_ed25519("나비@테스트").expect("생성");
        // 왕복: 만든 개인키를 다시 파싱할 수 있고, 공개키가 일치해야 한다.
        let parsed = PrivateKey::from_openssh(&pem).expect("재파싱");
        assert_eq!(parsed.algorithm(), Algorithm::Ed25519);
        assert!(pub_line.starts_with("ssh-ed25519 "), "{pub_line}");
        assert!(pub_line.ends_with("나비@테스트"), "코멘트 보존: {pub_line}");
        let (pem2, _) = generate_ed25519("").expect("생성2");
        assert_ne!(pem, pem2, "매번 새 키");
    }
}
