//! 환경 관리자 — **이 PC를 개발할 수 있는 상태로 만드는 한 곳.**
//!
//! ## 왜 도움말도 설정도 아닌가 (2026-08-25 사용자와 결론)
//!
//! 도움말은 *읽는* 곳이다. 반년 뒤에 gh를 다시 깔려고 도움말을 뒤지는 사람은 없다.
//! 설정은 *나비텀의 동작*을 바꾸는 곳인데, WSL이나 gh를 까는 것은 나비텀 동작이 아니라
//! **이 PC에 프로그램을 설치하는 일**이다. 결정적으로 설정 창 아래에는 "기본값으로
//! 되돌리기"가 있다 — 그 옆에 WSL 목록이 있으면 "이거 누르면 우분투도 지워지나?"라는
//! 질문이 반드시 생긴다. 그 질문이 생기는 자리는 틀린 자리다.
//!
//! 그래서 **도구 메뉴에서 여는 독립 창**이다. 도움말에는 설명만 남기고 여기로 보낸다.

use crate::aicli::ActionJob;
use crate::envstate::EnvScan;

/// 창의 살아 있는 상태.
pub(crate) struct EnvMgr {
    pub scan: EnvScan,
    /// 지금 돌고 있는 설치/제거 하나와 그 대상 이름.
    pub job: Option<(String, ActionJob)>,
    /// 마지막 결과 한 줄(성공/실패).
    pub note: Option<(bool, String)>,
    /// 설치 뒤 재검사가 필요한가.
    pub dirty: bool,
}

impl EnvMgr {
    pub fn new() -> Self {
        Self { scan: crate::envstate::scan(), job: None, note: None, dirty: false }
    }

    /// 다시 훑는다(설치가 끝났거나 사용자가 새로고침을 눌렀을 때).
    pub fn rescan(&mut self) {
        self.scan = crate::envstate::scan();
        self.dirty = false;
    }

    /// 한 번에 하나만 돌린다 — 동시에 두 설치 프로그램을 띄우면 서로의 파일을 밟는다.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// 작업을 시작한다. 이미 도는 게 있으면 무시한다.
    pub fn start(&mut self, label: impl Into<String>, script: String, first: String) {
        if self.busy() {
            return;
        }
        self.note = None;
        self.job = Some((label.into(), crate::envrun::start_script(script, first)));
    }

    /// 끝났으면 결과를 갈무리하고 재검사를 예약한다. 화면을 계속 다시 그려야 하면 true.
    pub fn poll(&mut self) -> bool {
        let Some((label, job)) = &self.job else { return false };
        let Ok(p) = job.lock() else { return false };
        if !p.done {
            return true;
        }
        let (ok, msg) = (p.success, p.message.clone());
        let label = label.clone();
        drop(p);
        if ok {
            // 설치 프로그램이 고친 PATH를 우리 프로세스에도 들여온다 — 안 그러면 방금 깐
            // 도구가 우리 눈에도, 새 pane에도 안 보인다(실제로 gh가 그랬다).
            crate::envpath::refresh();
            // 방금 깐 것이 '새 로컬 터미널' 목록에 바로 나타나야 한다.
            crate::menu::clear_shell_cache();
        }
        self.note = Some((ok, format!("{label}: {msg}")));
        self.job = None;
        self.dirty = true;
        true
    }

    /// 지금 그릴 진행 상태 (0~1, 라벨).
    pub fn progress(&self) -> Option<(f32, String)> {
        let (label, job) = self.job.as_ref()?;
        let p = job.lock().ok()?;
        Some((p.fraction, format!("{label} — {}", p.message)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn done_job(success: bool) -> ActionJob {
        Arc::new(Mutex::new(crate::aicli::ActionProgress {
            fraction: 1.0,
            message: "완료".into(),
            done: true,
            success,
            refresh_done: false,
        }))
    }

    /// 둘을 동시에 돌리면 설치 프로그램끼리 부딪힌다 — 두 번째 요청은 무시돼야 한다.
    #[test]
    fn only_one_job_runs_at_a_time() {
        let mut m = EnvMgr { scan: Arc::new(Mutex::new(Default::default())), job: None, note: None, dirty: false };
        m.job = Some(("gh".into(), done_job(true)));
        m.start("rg", "echo x".into(), "시작".into());
        assert_eq!(m.job.as_ref().unwrap().0, "gh", "두 번째가 첫 번째를 밀어냈다");
    }

    /// 끝나면 결과가 남고 재검사가 예약된다.
    #[test]
    fn a_finished_job_leaves_a_note_and_asks_for_a_rescan() {
        let mut m = EnvMgr { scan: Arc::new(Mutex::new(Default::default())), job: None, note: None, dirty: false };
        m.job = Some(("gh".into(), done_job(true)));
        assert!(m.poll());
        assert!(!m.busy());
        assert!(m.dirty, "설치 뒤에는 다시 훑어야 한다");
        let (ok, msg) = m.note.clone().unwrap();
        assert!(ok && msg.starts_with("gh: "), "{msg}");
    }

    /// 실패도 똑같이 남긴다 — 조용히 사라지면 사용자는 성공한 줄 안다.
    #[test]
    fn a_failure_is_reported_too() {
        let mut m = EnvMgr { scan: Arc::new(Mutex::new(Default::default())), job: None, note: None, dirty: false };
        m.job = Some(("winget".into(), done_job(false)));
        m.poll();
        assert_eq!(m.note.map(|n| n.0), Some(false));
    }
}
