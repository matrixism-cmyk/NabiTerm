//! 호스트키 사용자 확인 브리지 — 비동기 SSH 핸들러 ↔ UI 모달.
//!
//! 핸들러가 미지 호스트키를 만나면 `verify`로 Event::HostKeyPrompt를 UI에 보내고
//! oneshot으로 결정을 기다린다. UI가 Command::HostKeyDecision을 보내면 `resolve`가
//! 그 oneshot을 완료해 핸들러가 수락/거부를 진행한다.

use crossbeam_channel::Sender;
use nabi_proto::Event;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// id → 대기 중인 결정 회신 채널.
type Pending = Mutex<HashMap<u64, oneshot::Sender<bool>>>;

/// 오케스트레이터의 호스트키 확인기(verifier). connect_ssh_pane에 Arc로 넘긴다.
pub struct OrchVerifier {
    event_tx: Sender<Event>,
    pending: Pending,
    next: AtomicU64,
}

impl OrchVerifier {
    pub fn new(event_tx: Sender<Event>) -> Arc<Self> {
        Arc::new(Self {
            event_tx,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        })
    }

    /// 사용자 결정으로 대기 중인 프롬프트를 완료한다(Command::HostKeyDecision).
    pub fn resolve(&self, id: u64, accept: bool) {
        if let Ok(mut m) = self.pending.lock() {
            if let Some(tx) = m.remove(&id) {
                let _ = tx.send(accept);
            }
        }
    }
}

impl nabi_ssh::HostKeyVerify for OrchVerifier {
    fn verify(&self, info: nabi_ssh::HostKeyInfo) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut m) = self.pending.lock() {
            m.insert(id, tx);
        }
        let _ = self.event_tx.send(Event::HostKeyPrompt {
            id,
            host: info.host,
            port: info.port,
            algorithm: info.algorithm,
            fingerprint: info.fingerprint,
        });
        rx
    }
}
