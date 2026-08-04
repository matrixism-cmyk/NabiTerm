//! 호스트키 사용자 확인(TOFU 프롬프트) — 미지 호스트키를 UI에 확인 요청한다.

/// 사용자에게 보여줄 호스트키 정보.
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

/// 미지 호스트키 확인기. UI 스레드와 비동기로 수락/거부를 주고받는다.
/// 반환된 oneshot이 true면 신뢰(known_hosts 학습), false/취소면 연결 거부.
pub trait HostKeyVerify: Send + Sync {
    fn verify(&self, info: HostKeyInfo) -> tokio::sync::oneshot::Receiver<bool>;
}

pub type HostKeyVerifier = std::sync::Arc<dyn HostKeyVerify>;
