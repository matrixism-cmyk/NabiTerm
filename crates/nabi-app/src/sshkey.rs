//! SSH 키 파일명 기반 종류 힌트(B5) — 파일을 읽지 않고 OpenSSH 명명 관례로 추론한다.
//! 순수 함수(단위테스트). FIDO2/하드웨어 키(sk-)를 강조해 현대적 키 사용을 유도.

/// 키 힌트: (표시 라벨, 하드웨어 백업 여부, 레거시 여부).
pub(crate) struct KeyHint {
    pub label: &'static str,
    pub hardware: bool, // FIDO2/하드웨어 보안키(sk-)
    pub legacy: bool,   // rsa/dsa(약함·구식)
}

/// 키 경로(파일명)에서 종류를 추론한다. 알 수 없으면 None.
/// 관례: id_ed25519 / id_ed25519_sk / id_ecdsa_sk / id_rsa …
pub(crate) fn key_hint(path: &str) -> Option<KeyHint> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
    let hw = name.contains("_sk") || name.contains("-sk") || name.contains("sk_") || name.contains("fido");
    if name.contains("ed25519") {
        return Some(KeyHint { label: if hw { "ed25519-sk" } else { "ed25519" }, hardware: hw, legacy: false });
    }
    if name.contains("ecdsa") {
        return Some(KeyHint { label: if hw { "ecdsa-sk" } else { "ecdsa" }, hardware: hw, legacy: false });
    }
    if name.contains("rsa") {
        return Some(KeyHint { label: "rsa", hardware: false, legacy: true });
    }
    if name.contains("dsa") {
        return Some(KeyHint { label: "dsa", hardware: false, legacy: true });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_key_names() {
        let h = key_hint("~/.ssh/id_ed25519_sk").unwrap();
        assert_eq!((h.label, h.hardware, h.legacy), ("ed25519-sk", true, false));
        let h = key_hint("C:\\Users\\u\\.ssh\\id_ed25519").unwrap();
        assert_eq!((h.label, h.hardware), ("ed25519", false));
        let h = key_hint("id_ecdsa_sk").unwrap();
        assert_eq!((h.label, h.hardware), ("ecdsa-sk", true));
        let h = key_hint("/home/u/.ssh/id_rsa").unwrap();
        assert_eq!((h.label, h.legacy), ("rsa", true));
        assert!(key_hint("").is_none());
        assert!(key_hint("~/.ssh/mykey").is_none());
    }
}
