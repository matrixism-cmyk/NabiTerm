//! 진행률 — 순수 계산만. 시각은 밀리초 정수로 **받는다**(`Instant`를 쓰면 시험이 시간에 묶인다).

/// UI가 그리는 데 필요한 전부.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    /// 지금 파일이 몇 번째인가(1부터). 0이면 아직 파일이 정해지지 않았다.
    pub index: usize,
    /// 이번 전송의 파일 개수.
    pub count: usize,
    pub name: String,
    /// 이 파일에서 끝낸 바이트.
    pub done: u64,
    /// 이 파일의 전체 바이트.
    pub total: u64,
    /// 초당 바이트(최근 구간 기준). 아직 못 재면 0.
    pub bps: u64,
}

impl Progress {
    /// 0.0~1.0. 크기를 모르면 None(막대 대신 양·속도만 보여 준다).
    pub fn fraction(&self) -> Option<f32> {
        (self.total > 0).then(|| (self.done as f64 / self.total as f64).clamp(0.0, 1.0) as f32)
    }

    /// 남은 시간(초). 속도나 크기를 모르면 None.
    pub fn eta_secs(&self) -> Option<u64> {
        (self.bps > 0 && self.total > self.done).then(|| (self.total - self.done) / self.bps)
    }
}

/// 최근 구간의 전송 속도를 재는 자. 순간값이 튀지 않게 **창을 두고** 잰다.
#[derive(Debug, Clone)]
pub struct Rate {
    window_ms: u64,
    /// (시각ms, 그때까지 누적 바이트) — 창 밖으로 나간 표본은 버린다.
    marks: std::collections::VecDeque<(u64, u64)>,
}

impl Default for Rate {
    fn default() -> Self {
        Self::new(3000)
    }
}

impl Rate {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, marks: std::collections::VecDeque::new() }
    }

    /// 누적 바이트를 기록하고 지금 속도(B/s)를 돌려준다.
    pub fn push(&mut self, now_ms: u64, total_bytes: u64) -> u64 {
        self.marks.push_back((now_ms, total_bytes));
        let cutoff = now_ms.saturating_sub(self.window_ms);
        // 창보다 오래된 표본은 버리되, 기준점 하나는 남긴다(전부 버리면 잴 게 없다).
        while self.marks.len() > 2 && self.marks[1].0 < cutoff {
            self.marks.pop_front();
        }
        let (t0, b0) = *self.marks.front().unwrap_or(&(now_ms, total_bytes));
        let dt = now_ms.saturating_sub(t0);
        if dt == 0 {
            return 0;
        }
        total_bytes.saturating_sub(b0) * 1000 / dt
    }

    pub fn reset(&mut self) {
        self.marks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_and_eta() {
        let p = Progress { done: 50, total: 200, bps: 10, ..Progress::default() };
        assert_eq!(p.fraction(), Some(0.25));
        assert_eq!(p.eta_secs(), Some(15));
    }

    #[test]
    fn unknown_size_has_no_bar_and_no_eta() {
        let p = Progress { done: 50, total: 0, bps: 10, ..Progress::default() };
        assert_eq!(p.fraction(), None);
        assert_eq!(p.eta_secs(), None);
    }

    #[test]
    fn finished_file_has_no_eta() {
        let p = Progress { done: 200, total: 200, bps: 10, ..Progress::default() };
        assert_eq!(p.fraction(), Some(1.0));
        assert_eq!(p.eta_secs(), None);
    }

    #[test]
    fn rate_measures_over_the_window() {
        let mut r = Rate::new(3000);
        assert_eq!(r.push(0, 0), 0, "표본 하나로는 잴 수 없다");
        assert_eq!(r.push(1000, 1000), 1000);
        assert_eq!(r.push(2000, 3000), 1500, "2초에 3000바이트");
    }

    #[test]
    fn rate_forgets_old_samples() {
        let mut r = Rate::new(1000);
        r.push(0, 0);
        r.push(1000, 1_000_000); // 폭발적으로 빨랐던 구간
        r.push(2000, 1_001_000);
        r.push(3000, 1_002_000);
        assert!(r.push(4000, 1_003_000) < 5000, "옛 표본이 계속 속도를 부풀리면 안 된다");
    }

    #[test]
    fn rate_survives_a_stalled_clock() {
        let mut r = Rate::new(3000);
        r.push(500, 100);
        assert_eq!(r.push(500, 200), 0, "시간이 안 갔으면 나눗셈을 하지 않는다");
    }
}
