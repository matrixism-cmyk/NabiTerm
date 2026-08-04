//! 제어 권한 정책 v2 — off/ask/on + verb 그룹(read/act/inject)별 승인(CP-7).
//!
//! read(list/capture/wait/tail)는 ask에서도 무승인. act/inject는 요청자 pane별
//! **별도** 승인 집합(act 승인이 inject를 풀지 않음 — Kitty rc-permission 모델).

use crossbeam_channel::Sender;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

/// 제어 모드. off=전부 거부, ask=그룹별 1회 승인, on=항상 허용.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Off,
    Ask,
    On,
}

/// verb 그룹. Read=관찰, Act=상태 변경(비주입), Inject=입력 주입·종료급.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Read,
    Act,
    Inject,
}

impl Mode {
    /// 설정 문자열("off"/"ask"/"on") → Mode(미상은 Ask — 가장 보수적이되 사용가능).
    pub fn parse(s: &str) -> Mode {
        match s {
            "off" => Mode::Off,
            "on" => Mode::On,
            _ => Mode::Ask,
        }
    }
    fn to_u8(self) -> u8 {
        match self {
            Mode::Off => 0,
            Mode::Ask => 1,
            Mode::On => 2,
        }
    }
    fn from_u8(v: u8) -> Mode {
        match v {
            0 => Mode::Off,
            2 => Mode::On,
            _ => Mode::Ask,
        }
    }
}

/// 서버(제어 스레드)와 앱(UI 스레드)이 공유하는 권한 상태. 모두 Arc 복제로 공유.
#[derive(Clone)]
pub struct ControlPolicy {
    mode: Arc<AtomicU8>,
    /// act 그룹이 승인된 요청자 pane ID 집합.
    act: Arc<RwLock<HashSet<u64>>>,
    /// inject 그룹이 승인된 요청자 pane ID 집합(act와 별도).
    inject: Arc<RwLock<HashSet<u64>>>,
    /// ask 모드에서 미승인 요청 발생 시 앱에 (요청자, 그룹) 승인 요청.
    ask_tx: Sender<(u64, Group)>,
}

impl ControlPolicy {
    /// 정책 + 승인요청 수신기를 만든다(수신기는 앱이 매 프레임 drain).
    pub fn new(mode: Mode) -> (ControlPolicy, crossbeam_channel::Receiver<(u64, Group)>) {
        let (ask_tx, ask_rx) = crossbeam_channel::unbounded();
        let p = ControlPolicy {
            mode: Arc::new(AtomicU8::new(mode.to_u8())),
            act: Arc::new(RwLock::new(HashSet::new())),
            inject: Arc::new(RwLock::new(HashSet::new())),
            ask_tx,
        };
        (p, ask_rx)
    }

    pub fn mode(&self) -> Mode {
        Mode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    pub fn set_mode(&self, m: Mode) {
        self.mode.store(m.to_u8(), Ordering::Relaxed);
    }

    fn set_of(&self, g: Group) -> &Arc<RwLock<HashSet<u64>>> {
        match g {
            Group::Inject => &self.inject,
            _ => &self.act,
        }
    }

    /// 앱이 사용자 승인 시 호출 — 이후 그 pane의 해당 그룹 동작 허용.
    pub fn approve(&self, pane: u64, g: Group) {
        if let Ok(mut s) = self.set_of(g).write() {
            s.insert(pane);
        }
    }

    /// 승인 취소(설정 UI revoke).
    pub fn revoke(&self, pane: u64, g: Group) {
        if let Ok(mut s) = self.set_of(g).write() {
            s.remove(&pane);
        }
    }

    /// 현재 승인 현황 [(pane, 그룹)] — 설정 UI 표시용.
    pub fn approvals(&self) -> Vec<(u64, Group)> {
        let mut out = Vec::new();
        if let Ok(s) = self.act.read() {
            out.extend(s.iter().map(|p| (*p, Group::Act)));
        }
        if let Ok(s) = self.inject.read() {
            out.extend(s.iter().map(|p| (*p, Group::Inject)));
        }
        out.sort_by_key(|(p, g)| (*p, matches!(g, Group::Inject)));
        out
    }

    /// 그룹 동작 허용 여부. ask면 read는 항상 허용, act/inject는 pane별 승인,
    /// 미승인이면 앱에 (pane, 그룹) 승인 요청을 보내고 false.
    pub fn allow(&self, g: Group, from: Option<u64>) -> bool {
        match self.mode() {
            Mode::Off => false,
            Mode::On => true,
            Mode::Ask => {
                if g == Group::Read {
                    return true; // 관찰은 무승인(G4 — capture와 동급).
                }
                let p = from.unwrap_or(0);
                if self.set_of(g).read().map(|s| s.contains(&p)).unwrap_or(false) {
                    true
                } else {
                    let _ = self.ask_tx.send((p, g)); // 앱이 다이얼로그로 승인 유도.
                    false
                }
            }
        }
    }

    /// (v1 호환) 쓰기 동작 허용 — inject 기준(가장 보수적).
    pub fn allow_write(&self, from: Option<u64>) -> bool {
        self.allow(Group::Inject, from)
    }
}

#[cfg(test)]
mod tests {
    use super::{ControlPolicy, Group, Mode};

    #[test]
    fn mode_parse_defaults_to_ask() {
        assert_eq!(Mode::parse("off"), Mode::Off);
        assert_eq!(Mode::parse("on"), Mode::On);
        assert_eq!(Mode::parse("ask"), Mode::Ask);
        assert_eq!(Mode::parse("garbage"), Mode::Ask); // 미상은 보수적으로 Ask.
    }

    #[test]
    fn off_denies_on_allows_all() {
        let (p, _rx) = ControlPolicy::new(Mode::Off);
        assert!(!p.allow(Group::Read, Some(1)));
        assert!(!p.allow(Group::Inject, Some(1)));
        p.set_mode(Mode::On);
        assert!(p.allow(Group::Read, Some(1)) && p.allow(Group::Inject, Some(1)));
    }

    #[test]
    fn ask_read_free_act_needs_approval() {
        let (p, _rx) = ControlPolicy::new(Mode::Ask);
        assert!(p.allow(Group::Read, Some(7))); // 관찰은 무승인.
        assert!(!p.allow(Group::Act, Some(7))); // 미승인 → 거부.
        p.approve(7, Group::Act);
        assert!(p.allow(Group::Act, Some(7))); // 승인 후 허용.
    }

    #[test]
    fn act_approval_does_not_unlock_inject() {
        let (p, _rx) = ControlPolicy::new(Mode::Ask);
        p.approve(7, Group::Act);
        assert!(!p.allow(Group::Inject, Some(7))); // act 승인이 inject를 풀지 않음(분리 집합).
        p.approve(7, Group::Inject);
        assert!(p.allow(Group::Inject, Some(7)) && p.allow_write(Some(7)));
        p.revoke(7, Group::Inject);
        assert!(!p.allow(Group::Inject, Some(7))); // revoke 후 거부.
    }
}
