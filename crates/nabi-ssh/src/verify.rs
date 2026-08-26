//! 호스트키 사용자 확인(TOFU 프롬프트) — 미지 호스트키를 UI에 확인 요청한다.

/// 사용자에게 보여줄 호스트키 정보.
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    /// **전에 알던 키와 다르다**(known_hosts에 다른 지문이 적혀 있다).
    ///
    /// 중간자 공격과 서버 재설치는 겉으로 똑같이 생겼다 — 우리는 둘을 구별할 수 없다.
    /// 그래서 여기서 하는 일은 **판단이 아니라 알림**이다: 바뀌었다는 사실과 옛 줄 번호를
    /// 그대로 올려 보내고, 무엇을 할지는 사람이 정한다.
    pub changed: Option<ChangedKey>,
}

/// 알던 키가 바뀌었을 때의 자세한 사정.
pub struct ChangedKey {
    /// known_hosts에서 그 항목이 있는 줄(1부터).
    pub line: usize,
    /// 전에 알던 지문. 파일에서 읽지 못하면 비어 있다.
    pub old_fingerprint: String,
}

/// 호스트키 확인기(미지 호스트 · 바뀐 키 공용). UI 스레드와 비동기로 수락/거부를 주고받는다.
/// 반환된 oneshot이 true면 신뢰(known_hosts 학습), false/취소면 연결 거부.
pub trait HostKeyVerify: Send + Sync {
    fn verify(&self, info: HostKeyInfo) -> tokio::sync::oneshot::Receiver<bool>;
}

pub type HostKeyVerifier = std::sync::Arc<dyn HostKeyVerify>;
