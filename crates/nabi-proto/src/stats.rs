//! 원격 SSH 서버 리소스 통계(MobaXterm식 상태바 표시용) — 표시 모델 + 임계 판정.
//!
//! `/proc` 블롭 파싱은 [`crate::statsparse`](순수 함수, 단위테스트)가 담당한다.

/// 한 시점의 서버 리소스 스냅샷. CPU%·네트워크 속도는 연속 두 표본의 차이로 산출된다.
/// OS/커널 문자열을 담아 더 이상 Copy가 아니다(Clone).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerStats {
    pub cpu_pct: Option<f32>, // 0..100
    pub mem_used_kb: Option<u64>,
    pub mem_total_kb: Option<u64>,
    pub swap_used_kb: Option<u64>,
    pub swap_total_kb: Option<u64>,
    pub load1: Option<f32>,  // 1분 부하 평균
    pub load5: Option<f32>,  // 5분 부하 평균
    pub load15: Option<f32>, // 15분 부하 평균
    pub uptime_secs: Option<u64>,
    pub disk_used_kb: Option<u64>,
    pub disk_total_kb: Option<u64>,
    pub net_rx_bps: Option<u64>, // 바이트/초(수신)
    pub net_tx_bps: Option<u64>, // 바이트/초(송신)
    pub users: Option<u32>,      // 로그인 사용자 수(who)
    pub rtt_ms: Option<u32>,     // 통계 채널 왕복 지연(ms)
    pub os: Option<String>,      // 배포판/OS(PRETTY_NAME 또는 uname -s)
    pub kernel: Option<String>,  // 커널(uname -r)
    pub locale: Option<String>,  // 서버 로캘($LANG) — 한글 인코딩 3중문제 진단 보조
    pub top_proc: Option<String>, // CPU 최상위 프로세스("chrome 12%")
}

impl ServerStats {
    /// 채워진 항목이 하나도 없으면 true(표시 생략용).
    pub fn is_empty(&self) -> bool {
        self.cpu_pct.is_none()
            && self.mem_total_kb.is_none()
            && self.load1.is_none()
            && self.uptime_secs.is_none()
            && self.disk_total_kb.is_none()
            && self.net_rx_bps.is_none()
            && self.users.is_none()
            && self.os.is_none()
            && self.rtt_ms.is_none()
    }

    /// 호버 툴팁용 상세(OS·커널·접속자·RTT) — 한 줄에 다 안 들어가는 정보.
    pub fn detail(&self) -> String {
        let mut p: Vec<String> = Vec::new();
        if let Some(o) = &self.os {
            p.push(format!("OS: {o}"));
        }
        if let Some(k) = &self.kernel {
            p.push(format!("Kernel: {k}"));
        }
        if let Some(l) = &self.locale {
            p.push(format!("Locale: {l}"));
        }
        if let Some(tp) = &self.top_proc {
            p.push(format!("Top: {tp}"));
        }
        if let Some(u) = self.users {
            p.push(format!("Users: {u}"));
        }
        if let Some(r) = self.rtt_ms {
            p.push(format!("RTT: {r}ms"));
        }
        p.join("\n")
    }

    /// 메모리 사용률(%) — 임계 색상 판정용.
    pub fn mem_pct(&self) -> Option<f32> {
        pct(self.mem_used_kb, self.mem_total_kb)
    }

    /// 디스크 사용률(%).
    pub fn disk_pct(&self) -> Option<f32> {
        pct(self.disk_used_kb, self.disk_total_kb)
    }

    /// CPU/메모리/디스크 중 하나라도 thresh%를 넘으면 경보(빨강 강조용).
    pub fn alert(&self, thresh: f32) -> bool {
        self.cpu_pct.is_some_and(|v| v >= thresh)
            || self.mem_pct().is_some_and(|v| v >= thresh)
            || self.disk_pct().is_some_and(|v| v >= thresh)
    }

    /// 상태바용 요약("CPU 12% · MEM 3.2G/8.0G · 💽 12/50G · ↓1.2M/s ↑0.3M/s · load 0.45 · up 5d3h").
    pub fn summary(&self) -> String {
        let mut p: Vec<String> = Vec::new();
        if let Some(c) = self.cpu_pct {
            p.push(format!("CPU {c:.0}%"));
        }
        if let (Some(u), Some(t)) = (self.mem_used_kb, self.mem_total_kb) {
            p.push(format!("MEM {}/{}", human_kb(u), human_kb(t)));
        }
        // 스왑은 실제로 쓰일 때만 표시(0이면 생략 — 노이즈 방지).
        if let (Some(u), Some(t)) = (self.swap_used_kb, self.swap_total_kb) {
            if u > 0 && t > 0 {
                p.push(format!("SWP {}/{}", human_kb(u), human_kb(t)));
            }
        }
        if let (Some(u), Some(t)) = (self.disk_used_kb, self.disk_total_kb) {
            p.push(format!("\u{1f4bd} {}/{}", human_kb(u), human_kb(t)));
        }
        if let (Some(rx), Some(tx)) = (self.net_rx_bps, self.net_tx_bps) {
            p.push(format!("\u{2193}{} \u{2191}{}", human_bps(rx), human_bps(tx)));
        }
        if let Some(l1) = self.load1 {
            // 1·5·15분 추세를 함께(있을 때) — 1>5>15면 상승세. 표준 sysadmin 표기.
            let l5 = self.load5.map(|v| format!("/{v:.2}")).unwrap_or_default();
            let l15 = self.load15.map(|v| format!("/{v:.2}")).unwrap_or_default();
            p.push(format!("load {l1:.2}{l5}{l15}"));
        }
        if let Some(s) = self.uptime_secs {
            p.push(format!("up {}", human_uptime(s)));
        }
        if let Some(u) = self.users {
            p.push(format!("\u{1f465}{u}")); // 접속자 수
        }
        if let Some(r) = self.rtt_ms {
            p.push(format!("\u{27f2}{r}ms")); // 응답 지연
        }
        p.join(" \u{00b7} ")
    }
}

fn pct(used: Option<u64>, total: Option<u64>) -> Option<f32> {
    match (used, total) {
        (Some(u), Some(t)) if t > 0 => Some((u as f32 / t as f32 * 100.0).clamp(0.0, 100.0)),
        _ => None,
    }
}

/// kB → 사람이 읽는 용량("512M" / "3.2G").
pub fn human_kb(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 {
        format!("{:.1}G", mb / 1024.0)
    } else {
        format!("{mb:.0}M")
    }
}

/// 바이트/초 → "1.2M/s" / "12K/s" / "8B/s".
pub fn human_bps(bps: u64) -> String {
    let kb = bps as f64 / 1024.0;
    if kb >= 1024.0 {
        format!("{:.1}M/s", kb / 1024.0)
    } else if kb >= 1.0 {
        format!("{kb:.0}K/s")
    } else {
        format!("{bps}B/s")
    }
}

/// 초 → 가동시간("5d3h" / "3h12m" / "12m").
pub fn human_uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3_600, (secs % 3_600) / 60);
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{m}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_units() {
        assert_eq!(human_kb(512 * 1024), "512M");
        assert_eq!(human_kb(3 * 1024 * 1024 + 200 * 1024), "3.2G");
        assert_eq!(human_bps(2 * 1024 * 1024), "2.0M/s");
        assert_eq!(human_bps(512), "512B/s");
        assert_eq!(human_bps(4096), "4K/s");
        assert_eq!(human_uptime(5 * 86_400 + 3 * 3_600), "5d3h");
        assert_eq!(human_uptime(3 * 3_600 + 12 * 60), "3h12m");
        assert_eq!(human_uptime(45 * 60), "45m");
    }

    #[test]
    fn thresholds_and_alert() {
        let s = ServerStats {
            cpu_pct: Some(95.0),
            mem_used_kb: Some(7_000_000),
            mem_total_kb: Some(8_000_000),
            disk_used_kb: Some(10),
            disk_total_kb: Some(100),
            ..Default::default()
        };
        assert!((s.mem_pct().unwrap() - 87.5).abs() < 0.1);
        assert_eq!(s.disk_pct(), Some(10.0));
        assert!(s.alert(90.0)); // CPU 95 ≥ 90
        assert!(!s.alert(96.0)); // 아무것도 96 미만
        assert!(!ServerStats::default().alert(90.0));
    }

    #[test]
    fn summary_omits_unset_and_zero_swap() {
        assert_eq!(ServerStats::default().summary(), ""); // 값 없으면 빈 요약.
        let s = ServerStats {
            cpu_pct: Some(12.0),
            mem_used_kb: Some(1_000_000),
            mem_total_kb: Some(8_000_000),
            swap_used_kb: Some(0), // 스왑 미사용 → 생략.
            swap_total_kb: Some(2_000_000),
            ..Default::default()
        };
        let out = s.summary();
        assert!(out.contains("CPU 12%") && out.contains("MEM"));
        assert!(!out.contains("SWP")); // 스왑 0이면 노이즈 방지로 생략.
    }
}
