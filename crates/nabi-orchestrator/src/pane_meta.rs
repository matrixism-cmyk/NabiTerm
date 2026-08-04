//! pane 메타데이터 캐시 + 활동 상태머신(CP-6 — Herdr 모델).
//!
//! 이미 흐르는 신호(출력 damage·OSC 7/133·종료)만 집계한다 — 신규 신호 없음.
//! 상태는 시간 경과로 수동 전이되므로 질의 시점에 도출한다.

use std::time::Instant;

/// 출력 정지 후 idle로 보는 시간(ms).
const IDLE_MS: u128 = 2_000;
/// 명령 실행 중 출력 정지를 입력 대기(blocked)로 추정하는 시간(ms).
const BLOCKED_MS: u128 = 15_000;

/// 한 pane의 부가 상태(레지스트리 PaneView에 동봉).
pub struct PaneMeta {
    /// "local" | "ssh" (브라우저/SFTP 탭은 오케스트레이터 pane이 아님).
    pub kind: &'static str,
    /// 최신 작업 디렉터리(OSC 7).
    pub cwd: Option<String>,
    /// 명령 실행 중(OSC 133;C 수신 후 D 미수신).
    running: bool,
    /// 마지막 명령 종료 코드(OSC 133;D).
    pub last_exit: Option<i32>,
    last_output: Instant,
    cmd_started: Option<Instant>,
}

impl PaneMeta {
    pub fn new(kind: &'static str) -> Self {
        Self {
            kind,
            cwd: None,
            running: false,
            last_exit: None,
            last_output: Instant::now(),
            cmd_started: None,
        }
    }

    /// 출력 청크 도착.
    pub fn on_output(&mut self) {
        self.last_output = Instant::now();
    }

    /// OSC 133;C — 명령 실행 시작.
    pub fn on_command_started(&mut self) {
        self.running = true;
        self.cmd_started = Some(Instant::now());
    }

    /// OSC 133;D — 명령 종료(코드 보존).
    pub fn on_command_done(&mut self, exit: Option<i32>) {
        self.running = false;
        if exit.is_some() {
            self.last_exit = exit;
        }
    }

    /// OSC 7 — cwd 변경.
    pub fn on_cwd(&mut self, path: String) {
        self.cwd = Some(path);
    }

    /// 활동 상태 도출: working(실행/출력 중) · blocked(실행 중 출력 정지 — 입력
    /// 대기 추정) · idle. Blocked는 휴리스틱이므로 state_ms로 소비자가 재판정 가능.
    pub fn state(&self) -> &'static str {
        let quiet = self.last_output.elapsed().as_millis();
        if self.running {
            if quiet >= BLOCKED_MS {
                "blocked"
            } else {
                "working"
            }
        } else if quiet < IDLE_MS {
            "working"
        } else {
            "idle"
        }
    }

    /// 현 상태로 추정되는 경과(ms): working=명령 시작 이후, 그 외=마지막 출력 이후.
    pub fn state_ms(&self) -> u64 {
        let quiet = self.last_output.elapsed().as_millis() as u64;
        match self.state() {
            "working" => self
                .cmd_started
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(quiet),
            _ => quiet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transitions() {
        let mut m = PaneMeta::new("local");
        // 방금 출력 → working.
        m.on_output();
        assert_eq!(m.state(), "working");
        // 명령 시작 + 출력 정지 시뮬레이션: last_output을 과거로.
        m.on_command_started();
        m.last_output = Instant::now() - std::time::Duration::from_secs(20);
        assert_eq!(m.state(), "blocked");
        // 명령 종료 → idle(출력도 오래전).
        m.on_command_done(Some(0));
        assert_eq!(m.state(), "idle");
        assert_eq!(m.last_exit, Some(0));
        // 종료 직후 출력(프롬프트 재그리기) → 잠시 working, 2초 후 idle.
        m.on_output();
        assert_eq!(m.state(), "working");
    }
}
