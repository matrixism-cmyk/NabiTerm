//! 내장 스케줄러(C3) — cron/every/at 사양으로 pane 주입·셸 명령·알림을 실행한다.
//!
//! OpenClaw 스케줄러 벤치마킹에서 **모델 호출 페이로드는 의도적으로 뺐다**(비용 폭주 사고
//! 사례). 페이로드는 3종: send(대상 pane에 입력 주입 — 에이전트 주기 프롬프트),
//! command(숨김 셸 실행), notify(토스트). toml로 영속(재시작 생존), 연속 실패 10회면
//! 자동 비활성(무한 재시도 금지 — OpenClaw 백오프 규칙의 단순형).

use serde::{Deserialize, Serialize};

/// 등록된 잡 하나(toml 영속).
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Job {
    pub id: u64,
    pub name: String,
    /// 사양 원문("*/5 * * * *" | "every 15m" | "at 09:30") — 표시·재파싱용.
    pub spec: String,
    /// "send" | "command" | "notify".
    pub kind: String,
    pub payload: String,
    /// send 대상 pane 제목 부분 일치(비면 포커스 pane).
    #[serde(default)]
    pub pane_title: String,
    pub enabled: bool,
    /// 마지막 발화 분(unix분) — 같은 분 중복 발화 방지·every 기준점.
    #[serde(default)]
    pub last_fire_min: Option<i64>,
    /// 연속 실패 횟수(10이면 자동 비활성).
    #[serde(default)]
    pub fails: u32,
}

#[derive(Default, Serialize, Deserialize)]
struct SchedFile {
    jobs: Vec<Job>,
}

pub(crate) fn load(path: &std::path::Path) -> Vec<Job> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| toml::from_str::<SchedFile>(&t).ok())
        .map(|f| f.jobs)
        .unwrap_or_default()
}

pub(crate) fn save(path: &std::path::Path, jobs: &[Job]) {
    if let Ok(text) = toml::to_string_pretty(&SchedFile { jobs: jobs.to_vec() }) {
        let _ = std::fs::write(path, text);
    }
}

impl crate::app::NabiApp {
    /// 매 프레임 호출(내부 2초 스로틀 — 분 granularity라 충분).
    pub(crate) fn tick_scheduler(&mut self) {
        if self.sched_last_tick.elapsed() < std::time::Duration::from_secs(2) {
            return;
        }
        self.sched_last_tick = std::time::Instant::now();
        let now = chrono::Local::now();
        let cur_min = now.timestamp() / 60;
        let mut changed = false;
        let mut fired: Vec<usize> = Vec::new();
        for (i, job) in self.schedules.iter().enumerate() {
            if !job.enabled {
                continue;
            }
            let Ok(spec) = crate::schedspec::parse(&job.spec) else { continue };
            if crate::schedspec::due(&spec, &now, job.last_fire_min) {
                fired.push(i);
            }
        }
        for i in fired {
            let job = self.schedules[i].clone();
            let ok = self.run_job(&job);
            let j = &mut self.schedules[i];
            j.last_fire_min = Some(cur_min);
            j.fails = if ok { 0 } else { j.fails + 1 };
            if j.fails >= 10 {
                j.enabled = false; // 자동 비활성 — 조용한 무한 실패 금지.
                let name = j.name.clone();
                self.notify = Some((format!("\u{23f0} 스케줄 '{name}' 10회 연속 실패 — 비활성화"), std::time::Instant::now()));
            }
            changed = true;
        }
        if changed {
            save(&self.schedules_path, &self.schedules);
        }
    }

    /// 잡 실행. 반환=성공(실패 카운트 기준).
    fn run_job(&mut self, job: &Job) -> bool {
        match job.kind.as_str() {
            "send" => {
                let pane = if job.pane_title.is_empty() {
                    self.focused_pane()
                } else {
                    self.orch.panes.read().ok().and_then(|m| {
                        m.iter().find(|(_, v)| v.title.contains(&job.pane_title)).map(|(p, _)| *p)
                    })
                };
                match pane {
                    Some(p) => {
                        let mut data = job.payload.clone().into_bytes();
                        data.push(b'\r');
                        self.orch.send(nabi_proto::Command::WriteInput { pane: p, data: bytes::Bytes::from(data) });
                        true
                    }
                    None => false, // 대상 없음 = 실패(연속되면 자동 비활성).
                }
            }
            "command" => {
                use std::os::windows::process::CommandExt;
                std::process::Command::new("powershell")
                    .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &job.payload])
                    .creation_flags(0x0800_0000)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .is_ok()
            }
            _ => {
                self.notify = Some((format!("\u{23f0} {}", job.payload), std::time::Instant::now()));
                true
            }
        }
    }

    /// 잡 추가(CLI 경로). 사양이 틀리면 Err.
    pub(crate) fn add_schedule(&mut self, name: String, spec: String, kind: String, payload: String, pane_title: String) -> Result<(), String> {
        let path = self.schedules_path.clone();
        add(&mut self.schedules, &path, name, spec, kind, payload, pane_title)
    }
}

/// 잡 추가(UI/CLI 공용 — 등록 전에 사양을 검증해 조용히 죽는 잡을 막는다).
#[allow(clippy::too_many_arguments)]
pub(crate) fn add(
    jobs: &mut Vec<Job>,
    path: &std::path::Path,
    name: String,
    spec: String,
    kind: String,
    payload: String,
    pane_title: String,
) -> Result<(), String> {
    crate::schedspec::parse(&spec)?;
    let id = jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
    jobs.push(Job { id, name, spec, kind, payload, pane_title, enabled: true, last_fire_min: None, fails: 0 });
    save(path, jobs);
    Ok(())
}
