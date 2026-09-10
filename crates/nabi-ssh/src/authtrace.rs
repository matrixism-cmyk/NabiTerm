//! **무엇을 시도했고 어떻게 됐나** — 실패 화면에 그대로 적을 기록.
//!
//! ## 왜 필요한가
//!
//! 2026-09-10에 여러 방법을 차례로 시도하게 만들면서([`crate::authchain`]) 화면은 그대로
//! 두었다. 그 순간 **실패 화면이 거짓말을 하기 시작했다** — 목록에는 키 이름이 죽 뜨는데,
//! 서버가 `publickey` 를 안 받아 한 번도 안 던진 것과 던졌는데 거절당한 것이 똑같이
//! 보였다. 사용자는 있지도 않은 문제를 파게 된다.
//!
//! 기능을 더할 때 **그 기능을 설명하는 화면도 같이 고쳐야 한다.** 안 고치면 화면이
//! 조용히 옛날 이야기를 계속한다.
//!
//! ## 왜 pane 별 등록부인가
//!
//! 인증은 `run` 안 깊은 곳에서 일어나고, 실패 화면은 그 밖에서 그린다. 사이에 값을
//! 실어 나를 길이 없다(`russh::Error` 는 우리 것이 아니라 짐을 실을 수 없다).
//! 협상된 암호를 배지에 띄우는 [`crate::kexinfo`] 가 같은 사정으로 같은 모양을 쓴다.

use nabi_types::PaneId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 한 번의 시도가 어떻게 끝났나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 서버가 받아 줬다.
    Ok,
    /// 서버가 거절했다.
    Rejected,
    /// 서버에 던지지도 못했다(암호 걸린 키를 못 열었다 등).
    ///
    /// **이것은 서버 이야기가 아니다.** 섞어서 보여 주면 사용자가 서버 설정을 뒤진다.
    NotSent,
    /// 서버가 그 방법을 안 받는다고 해서 아예 건너뛰었다.
    Skipped,
}

/// 시도 한 줄: (무엇으로, 어떻게 됐나).
pub type Step = (String, Outcome);

fn registry() -> &'static Mutex<HashMap<PaneId, Vec<Step>>> {
    static REG: OnceLock<Mutex<HashMap<PaneId, Vec<Step>>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 한 줄 남긴다. 잠금이 깨져 있으면 그냥 넘어간다 — 기록 때문에 접속이 죽으면 안 된다.
pub fn push(pane: PaneId, what: impl Into<String>, how: Outcome) {
    if let Ok(mut m) = registry().lock() {
        m.entry(pane).or_default().push((what.into(), how));
    }
}

/// 이 pane 의 기록을 **가져가며 비운다**(실패 화면이 한 번 읽고 끝낸다).
pub fn take(pane: PaneId) -> Vec<Step> {
    registry().lock().ok().and_then(|mut m| m.remove(&pane)).unwrap_or_default()
}

/// 세션이 끝났으면 지운다 — 붙는 데 성공한 pane 의 기록이 남아 있을 이유가 없다.
pub fn clear(pane: PaneId) {
    if let Ok(mut m) = registry().lock() {
        m.remove(&pane);
    }
}

/// 결과를 사람 말로 옮길 i18n 키.
pub fn key_of(o: Outcome) -> &'static str {
    match o {
        Outcome::Ok => "ssh.diag.step.ok",
        Outcome::Rejected => "ssh.diag.step.rejected",
        Outcome::NotSent => "ssh.diag.step.notsent",
        Outcome::Skipped => "ssh.diag.step.skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(n: u64) -> PaneId {
        PaneId::new(n)
    }

    #[test]
    fn 남긴_순서대로_돌려준다() {
        let p = pane(9001);
        clear(p);
        push(p, "id_rsa", Outcome::Rejected);
        push(p, "agent", Outcome::Ok);
        let got = take(p);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("id_rsa".to_string(), Outcome::Rejected));
        assert_eq!(got[1].1, Outcome::Ok);
    }

    /// **한 번 읽으면 비워진다.** 안 비우면 다음 실패에 옛 기록이 섞여 나온다 —
    /// 그건 아무 기록도 없는 것보다 나쁘다(틀린 곳을 파게 만든다).
    #[test]
    fn 읽으면_비워진다() {
        let p = pane(9002);
        clear(p);
        push(p, "id_ed25519", Outcome::NotSent);
        assert_eq!(take(p).len(), 1);
        assert!(take(p).is_empty(), "두 번째 읽기에 옛 기록이 남았다");
    }

    /// pane 끼리 섞이지 않는다 — 여러 서버에 동시에 붙는 것이 이 프로그램의 일이다.
    #[test]
    fn pane_끼리_섞이지_않는다() {
        let (a, b) = (pane(9003), pane(9004));
        clear(a);
        clear(b);
        push(a, "a키", Outcome::Rejected);
        push(b, "b키", Outcome::Ok);
        assert_eq!(take(a)[0].0, "a키");
        assert_eq!(take(b)[0].0, "b키");
    }

    /// 결과마다 **다른** 말이 있어야 한다 — 겹치면 구별하려고 만든 뜻이 없다.
    #[test]
    fn 결과마다_다른_말이_있다() {
        let all = [Outcome::Ok, Outcome::Rejected, Outcome::NotSent, Outcome::Skipped];
        let keys: Vec<&str> = all.iter().map(|o| key_of(*o)).collect();
        let uniq: std::collections::HashSet<_> = keys.iter().collect();
        assert_eq!(uniq.len(), keys.len(), "겹치는 말이 있다: {keys:?}");
    }
}
