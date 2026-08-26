//! AI CLI 업데이트 — 최신 버전 조회, 수동 업데이트, 시작 시 자동 업데이트.
//!
//! 탐지·설치·제거는 [`crate::aicli`], 버전 비교와 통로 판정은 [`crate::aicliver`].

use crate::aicli::{install_npm_cli, run_ps, set_progress, ActionJob, ActionProgress};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 설치된 CLI를 최신으로 올린다. 설치할 때 쓴 통로를 그대로 따른다(두 벌 설치 방지).
pub(crate) fn start_update(id: &str, path: PathBuf) -> std::io::Result<ActionJob> {
    let Some(ch) = crate::aicliver::channel(id, &path) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "자동 업데이트를 지원하지 않는 CLI",
        ));
    };
    let job = Arc::new(Mutex::new(ActionProgress {
        message: "업데이트 준비 중…".into(),
        ..Default::default()
    }));
    let worker = job.clone();
    let name = id.to_string();
    std::thread::spawn(move || {
        let out = match ch {
            crate::aicliver::Channel::Npm(pkg) => {
                install_npm_cli(&worker, &name, &format!("{pkg}@latest"))
            }
            crate::aicliver::Channel::Native => {
                set_progress(&worker, 0.5, format!("{name} 업데이트 중…"));
                run_ps(&format!("& '{}' update", path.display()))
            }
        };
        crate::aicli::finish(&worker, out);
    });
    Ok(job)
}

/// 자동 업데이트 진행 상태. `done`을 따로 두는 이유는 "아직 도는 중"과 "올릴 게 없었다"를
/// 구분하기 위해서다 — 둘을 뭉뚱그리면 알림을 영영 못 내리거나 헛알림을 낸다.
#[derive(Default)]
pub(crate) struct AutoResult {
    pub done: bool,
    pub updated: Vec<String>,
}

pub(crate) type AutoJob = Arc<Mutex<AutoResult>>;

/// 설치된 AI CLI 중 뒤처진 것을 조용히 최신으로 올린다(설정에서 켠 경우에만 호출).
///
/// 하나씩 순차로 처리한다 — npm 전역 설치는 동시에 돌리면 서로의 파일을 밟는다.
pub(crate) fn start_auto_update() -> AutoJob {
    let out: AutoJob = Arc::new(Mutex::new(AutoResult::default()));
    let worker = out.clone();
    std::thread::spawn(move || {
        let mut done: Vec<String> = Vec::new();
        for cli in crate::aicli::detect_all() {
            let (Some(path), Some(ver)) = (cli.path.clone(), cli.version.clone()) else {
                continue;
            };
            let Some(latest) = crate::aicliver::npm_package(cli.id)
                .and_then(crate::aicliver::latest_npm)
                .filter(|l| crate::aicliver::is_outdated(&ver, l))
            else {
                continue;
            };
            if let Ok(job) = start_update(cli.id, path) {
                if wait_done(&job) {
                    done.push(format!("{} {latest}", cli.name));
                }
            }
        }
        if let Ok(mut s) = worker.lock() {
            s.updated = done;
            s.done = true;
        }
    });
    out
}

/// 작업이 끝날 때까지 기다렸다 성공 여부를 돌려준다(최대 10분 — 걸려도 스레드를 남기지 않게).
fn wait_done(job: &ActionJob) -> bool {
    for _ in 0..3000 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if let Ok(p) = job.lock() {
            if p.done {
                return p.success;
            }
        }
    }
    false
}

/// 배포된 최신 버전 조회 결과(CLI id → 최신 버전). 조회가 끝나기 전에는 None.
pub(crate) type LatestJob = Arc<Mutex<Option<Vec<(String, String)>>>>;

/// 최신 버전을 백그라운드에서 조회한다(UI 스레드에서 네트워크를 타지 않도록).
pub(crate) fn start_latest_check(ids: Vec<String>) -> LatestJob {
    let job: LatestJob = Arc::new(Mutex::new(None));
    let worker = job.clone();
    std::thread::spawn(move || {
        let found = ids
            .iter()
            .filter_map(|id| {
                let pkg = crate::aicliver::npm_package(id)?;
                Some((id.clone(), crate::aicliver::latest_npm(pkg)?))
            })
            .collect();
        if let Ok(mut slot) = worker.lock() {
            *slot = Some(found);
        }
    });
    job
}


/// 하루 — 자동 확인 주기. 켤 때마다가 아니라 하루 한 번이면 충분하다.
const DAY_SECS: i64 = 24 * 60 * 60;

impl crate::app::NabiApp {
    /// 시작 시 1회: 설정이 켜져 있고 마지막 확인이 하루보다 오래됐으면 자동 업데이트를 건다.
    pub(crate) fn start_ai_cli_auto_update(&mut self) {
        let now = crate::updatemodal::now_unix();
        let cfg = &self.config.terminal;
        // 자동 갱신도 **묻지 않은 호출**이다 — 오프라인 모드면 하지 않는다.
        if !crate::egress::may_call_unattended() {
            return;
        }
        if !cfg.ai_cli_auto_update || now - cfg.ai_cli_checked_at < DAY_SECS {
            return;
        }
        // 확인 시각은 결과와 무관하게 먼저 적어 둔다 — 실패해도 켤 때마다 다시 시도하지 않도록.
        self.config.terminal.ai_cli_checked_at = now;
        let _ = nabi_config::save(&self.config_path, &self.config);
        self.ai_cli_auto = Some(crate::aicliupd::start_auto_update());
    }

    /// 자동 업데이트가 끝났으면 올린 것만 토스트로 한 번 알리고 작업을 놓는다.
    pub(crate) fn poll_ai_cli_auto_update(&mut self) {
        let Some(job) = self.ai_cli_auto.clone() else { return };
        let Some(updated) = job.lock().ok().filter(|r| r.done).map(|r| r.updated.clone()) else {
            return;
        };
        self.ai_cli_auto = None;
        if !updated.is_empty() {
            let msg = format!("\u{2b06} AI CLI: {}", updated.join(", "));
            self.notify = Some((msg, std::time::Instant::now()));
        }
    }
}
