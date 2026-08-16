//! 협상된 키 교환(KEX)·암호 기록 — pane별 레지스트리(T1-2 PQ 배지).
//!
//! russh 0.62의 `kex_done` 훅이 채워 준다. UI(상태바)는 `get`으로 읽기만 하고,
//! 세션이 끝나면 `clear`로 지운다. rekey가 일어나면 최신 값으로 덮인다.

use nabi_types::PaneId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 한 연결의 협상 결과. 점프 호스트 경유라면 **목적지** 연결의 값이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KexInfo {
    /// 키 교환 알고리즘 이름(예: `mlkem768x25519-sha256`).
    pub kex: String,
    /// 대칭 암호 이름(예: `chacha20-poly1305@openssh.com`).
    pub cipher: String,
}

impl KexInfo {
    /// 포스트퀀텀 하이브리드 KEX인가 — ML-KEM 계열과 (러시 미지원이지만 대비) sntrup761.
    pub fn is_pq(&self) -> bool {
        is_pq_kex(&self.kex)
    }
}

/// KEX 이름만으로 PQ 여부 판정(순수 — 테스트 대상).
pub fn is_pq_kex(kex: &str) -> bool {
    kex.starts_with("mlkem") || kex.starts_with("sntrup")
}

/// 핸들러가 협상 결과를 써 넣는 슬롯. 연결 수립 코드가 만들어 핸들러에 쥐여 준다.
pub type KexSlot = Arc<Mutex<Option<KexInfo>>>;

pub fn new_slot() -> KexSlot {
    Arc::new(Mutex::new(None))
}

fn registry() -> &'static Mutex<HashMap<PaneId, KexInfo>> {
    static REG: OnceLock<Mutex<HashMap<PaneId, KexInfo>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// pane의 협상 결과를 기록한다(재협상 시 덮어씀).
pub fn set(pane: PaneId, info: KexInfo) {
    if let Ok(mut m) = registry().lock() {
        m.insert(pane, info);
    }
}

/// pane의 협상 결과(없으면 None — 로컬 pane이거나 아직 협상 전).
pub fn get(pane: PaneId) -> Option<KexInfo> {
    registry().lock().ok().and_then(|m| m.get(&pane).cloned())
}

/// 세션 종료 시 제거(죽은 pane의 배지 잔상 방지).
pub fn clear(pane: PaneId) {
    if let Ok(mut m) = registry().lock() {
        m.remove(&pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pq_detection() {
        assert!(is_pq_kex("mlkem768x25519-sha256"));
        assert!(is_pq_kex("sntrup761x25519-sha512@openssh.com"));
        assert!(!is_pq_kex("curve25519-sha256"));
        assert!(!is_pq_kex("diffie-hellman-group14-sha256"));
        assert!(!is_pq_kex(""));
    }

    #[test]
    fn registry_roundtrip() {
        let pane = PaneId::new(u64::MAX - 7); // 실사용과 겹치지 않는 키.
        assert_eq!(get(pane), None);
        set(pane, KexInfo { kex: "mlkem768x25519-sha256".into(), cipher: "aes256-gcm@openssh.com".into() });
        let got = get(pane).unwrap();
        assert!(got.is_pq());
        // 재협상은 덮어쓴다.
        set(pane, KexInfo { kex: "curve25519-sha256".into(), cipher: "aes256-gcm@openssh.com".into() });
        assert!(!get(pane).unwrap().is_pq());
        clear(pane);
        assert_eq!(get(pane), None);
    }
}
