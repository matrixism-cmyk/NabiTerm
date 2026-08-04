//! `/proc`·`df` 통계 블롭 파싱(순수 함수, 단위테스트). 폴러는 마커(@CPU 등) 출력을 모아 전달한다.

use crate::stats::ServerStats;

/// CPU%·네트워크 속도 산출용 직전 표본.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PollSample {
    pub cpu: Option<CpuSample>,
    pub net_rx: Option<u64>, // 누적 수신 바이트
    pub net_tx: Option<u64>, // 누적 송신 바이트
}

/// `/proc/stat` 첫 줄의 총합/유휴 누적.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuSample {
    pub total: u64,
    pub idle: u64,
}

/// `cpu user nice system idle iowait ...` → 총합/유휴(idle+iowait).
pub fn parse_cpu_sample(line: &str) -> Option<CpuSample> {
    let mut it = line.split_whitespace();
    if it.next()? != "cpu" {
        return None;
    }
    let v: Vec<u64> = it.filter_map(|t| t.parse().ok()).collect();
    if v.len() < 4 {
        return None;
    }
    Some(CpuSample { total: v.iter().sum(), idle: v[3] + v.get(4).copied().unwrap_or(0) })
}

/// 연속 두 표본으로 CPU 사용률(%). 변화 없으면 None.
pub fn cpu_pct(prev: CpuSample, cur: CpuSample) -> Option<f32> {
    let dt = cur.total.checked_sub(prev.total)?;
    let di = cur.idle.checked_sub(prev.idle)?;
    if dt == 0 {
        return None;
    }
    Some((dt.saturating_sub(di) as f32 / dt as f32 * 100.0).clamp(0.0, 100.0))
}

/// `/proc/meminfo` → (사용중 kB, 전체 kB). 사용중=Total-Available.
pub fn parse_meminfo(text: &str) -> (Option<u64>, Option<u64>) {
    let field = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let (total, avail) = (field("MemTotal:"), field("MemAvailable:"));
    let used = match (total, avail) {
        (Some(t), Some(a)) => Some(t.saturating_sub(a)),
        _ => None,
    };
    (used, total)
}

/// `/proc/meminfo` → (스왑 사용중 kB, 전체 kB). 사용중=SwapTotal-SwapFree.
pub fn parse_swap(text: &str) -> (Option<u64>, Option<u64>) {
    let field = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let (total, free) = (field("SwapTotal:"), field("SwapFree:"));
    let used = match (total, free) {
        (Some(t), Some(f)) => Some(t.saturating_sub(f)),
        _ => None,
    };
    (used, total)
}

/// `/proc/uptime` 첫 숫자(초).
pub fn parse_uptime(text: &str) -> Option<u64> {
    text.split_whitespace().next()?.split('.').next()?.parse().ok()
}

/// `ps -eo comm,pcpu --sort=-pcpu` 최상위 줄("chrome 12.3") → "chrome 12%". 형식 아니면 None.
pub fn parse_top_proc(text: &str) -> Option<String> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let pct = line.split_whitespace().last()?;
    let name = line[..line.len() - pct.len()].trim();
    let pctn: f32 = pct.parse().ok()?;
    (!name.is_empty()).then(|| format!("{name} {pctn:.0}%"))
}

/// `/proc/loadavg` 첫 숫자(1분 평균).
pub fn parse_loadavg(text: &str) -> Option<f32> {
    text.split_whitespace().next()?.parse().ok()
}

/// `/proc/loadavg`의 1·5·15분 평균(추세 표시용).
pub fn parse_loadavg3(text: &str) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut it = text.split_whitespace().map(|s| s.parse::<f32>().ok());
    (it.next().flatten(), it.next().flatten(), it.next().flatten())
}

/// `df -P -k <mount>` → (사용 kB, 전체 kB). 헤더 다음 데이터 줄의 1024-blocks/Used.
pub fn parse_disk(text: &str) -> (Option<u64>, Option<u64>) {
    for line in text.lines() {
        if line.starts_with("Filesystem") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 3 {
            // [fs, 1024-blocks(total), used, available, capacity, mount]
            return (f[2].parse().ok(), f[1].parse().ok());
        }
    }
    (None, None)
}

/// `/proc/net/dev` → (누적 rx, tx) 바이트. `lo`(루프백)는 제외하고 모든 인터페이스 합산.
pub fn parse_netdev(text: &str) -> Option<(u64, u64)> {
    let (mut rx, mut tx, mut any) = (0u64, 0u64, false);
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else { continue };
        if name.trim() == "lo" {
            continue;
        }
        let f: Vec<u64> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        // rx_bytes=0, tx_bytes=8 (rx: bytes packets errs drop fifo frame compressed multicast | tx: bytes ...)
        if let (Some(&r), Some(&t)) = (f.first(), f.get(8)) {
            rx += r;
            tx += t;
            any = true;
        }
    }
    any.then_some((rx, tx))
}

/// 마커(@CPU/@MEM/@UP/@LOAD/@DISK/@NET)로 구분된 블롭을 파싱. prev=직전 표본(CPU%/속도용),
/// interval_secs=폴링 주기(네트워크 속도 산출). 반환=(통계, 이번 표본).
pub fn parse_blob(raw: &str, prev: Option<PollSample>, interval_secs: u64) -> (ServerStats, PollSample) {
    let mut sec: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    let mut key = "";
    for line in raw.lines() {
        match line.trim() {
            "@CPU" => key = "cpu",
            "@MEM" => key = "mem",
            "@UP" => key = "up",
            "@LOAD" => key = "load",
            "@DISK" => key = "disk",
            "@NET" => key = "net",
            "@USERS" => key = "users",
            "@OS" => key = "os",
            "@LOCALE" => key = "locale",
            "@PROC" => key = "proc",
            other if !key.is_empty() => {
                let e = sec.entry(key).or_default();
                e.push_str(other);
                e.push('\n');
            }
            _ => {}
        }
    }
    let cpu = sec.get("cpu").and_then(|s| parse_cpu_sample(s.trim()));
    let (mem_used_kb, mem_total_kb) = sec.get("mem").map(|s| parse_meminfo(s)).unwrap_or((None, None));
    let (swap_used_kb, swap_total_kb) = sec.get("mem").map(|s| parse_swap(s)).unwrap_or((None, None));
    let (disk_used_kb, disk_total_kb) = sec.get("disk").map(|s| parse_disk(s)).unwrap_or((None, None));
    let net = sec.get("net").and_then(|s| parse_netdev(s));
    let sample = PollSample { cpu, net_rx: net.map(|n| n.0), net_tx: net.map(|n| n.1) };
    // 네트워크 속도 = (현재누적 - 직전누적) / 주기.
    let speed = |cur: Option<u64>, prv: Option<u64>| -> Option<u64> {
        match (cur, prv, interval_secs) {
            (Some(c), Some(p), s) if s > 0 => Some(c.saturating_sub(p) / s),
            _ => None,
        }
    };
    let (load1, load5, load15) = sec.get("load").map_or((None, None, None), |s| parse_loadavg3(s));
    let stats = ServerStats {
        cpu_pct: match (prev.and_then(|p| p.cpu), cpu) {
            (Some(p), Some(c)) => cpu_pct(p, c),
            _ => None,
        },
        mem_used_kb,
        mem_total_kb,
        swap_used_kb,
        swap_total_kb,
        load1,
        load5,
        load15,
        uptime_secs: sec.get("up").and_then(|s| parse_uptime(s)),
        disk_used_kb,
        disk_total_kb,
        net_rx_bps: speed(sample.net_rx, prev.and_then(|p| p.net_rx)),
        net_tx_bps: speed(sample.net_tx, prev.and_then(|p| p.net_tx)),
        users: sec.get("users").and_then(|s| s.trim().parse().ok()),
        rtt_ms: None, // 폴러가 왕복 측정으로 채운다.
        os: sec.get("os").and_then(|s| s.lines().next()).map(|l| l.trim().to_string()).filter(|s| !s.is_empty()),
        kernel: sec.get("os").and_then(|s| s.lines().nth(1)).map(|l| l.trim().to_string()).filter(|s| !s.is_empty()),
        locale: sec.get("locale").and_then(|s| s.lines().next()).map(|l| l.trim().to_string()).filter(|s| !s.is_empty()),
        top_proc: sec.get("proc").and_then(|s| parse_top_proc(s)),
    };
    (stats, sample)
}

/// 폴러가 원격에서 실행하는 한 줄 명령(마커 출력). /proc·df 없는 서버는 빈/부분 출력.
/// @USERS=접속자 수(who), @OS=배포판(PRETTY_NAME 또는 uname -s) + 커널(uname -r).
pub const STAT_CMD: &str = "echo @CPU; grep '^cpu ' /proc/stat; echo @MEM; \
grep -E '^(Mem(Total|Available)|Swap(Total|Free))' /proc/meminfo; echo @UP; cat /proc/uptime; \
echo @LOAD; cat /proc/loadavg; echo @DISK; df -P -k /; echo @NET; cat /proc/net/dev; \
echo @USERS; who 2>/dev/null | wc -l; echo @OS; \
(. /etc/os-release 2>/dev/null; echo \"${PRETTY_NAME:-$(uname -s)}\"); uname -r; \
echo @LOCALE; echo \"${LANG:-$(locale 2>/dev/null | sed -n 's/^LANG=//p')}\"; \
echo @PROC; ps -eo comm,pcpu --sort=-pcpu --no-headers 2>/dev/null | head -1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_pct_delta() {
        let a = parse_cpu_sample("cpu 100 0 50 800 50 0 0").unwrap();
        assert_eq!((a.total, a.idle), (1000, 850));
        let b = parse_cpu_sample("cpu 200 0 100 1600 100 0 0").unwrap();
        assert!((cpu_pct(a, b).unwrap() - 15.0).abs() < 0.01);
        assert_eq!(cpu_pct(a, a), None);
    }

    #[test]
    fn mem_disk_net() {
        assert_eq!(
            parse_meminfo("MemTotal: 8000000 kB\nMemAvailable: 5000000 kB"),
            (Some(3_000_000), Some(8_000_000))
        );
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted\n/dev/sda1 50000000 12000000 38000000 24% /";
        assert_eq!(parse_disk(df), (Some(12_000_000), Some(50_000_000)));
        let dev = "Inter-|Receive\nface |bytes ...\n  lo: 999 0 0 0 0 0 0 0 999 0\n eth0: 1000 5 0 0 0 0 0 0 2000 9";
        assert_eq!(parse_netdev(dev), Some((1000, 2000))); // lo 제외
    }

    #[test]
    fn loadavg3_parsing() {
        assert_eq!(parse_loadavg3("0.45 0.52 0.60 1/234 5678"), (Some(0.45), Some(0.52), Some(0.60)));
        assert_eq!(parse_loadavg3("0.10"), (Some(0.10), None, None)); // 부분 출력 허용.
        assert_eq!(parse_loadavg3(""), (None, None, None));
    }

    #[test]
    fn top_proc_parsing() {
        assert_eq!(parse_top_proc("chrome 12.3\n"), Some("chrome 12%".to_string()));
        assert_eq!(parse_top_proc("my proc 5.0"), Some("my proc 5%".to_string())); // 이름에 공백 허용
        assert_eq!(parse_top_proc(""), None);
        assert_eq!(parse_top_proc("noPctHere"), None);
    }

    #[test]
    fn swap_parsing() {
        let mi = "MemTotal: 8000000 kB\nMemAvailable: 5000000 kB\nSwapTotal: 2000000 kB\nSwapFree: 1500000 kB";
        assert_eq!(parse_swap(mi), (Some(500_000), Some(2_000_000)));
        assert_eq!(parse_swap("MemTotal: 1 kB"), (None, None)); // 스왑 항목 없음
        // 같은 meminfo 블롭에서 RAM·스왑 동시 파싱.
        let (s, _) = parse_blob("@MEM\nMemTotal: 8000000 kB\nMemAvailable: 5000000 kB\nSwapTotal: 2000000 kB\nSwapFree: 1500000 kB\n", None, 3);
        assert_eq!((s.mem_used_kb, s.swap_used_kb), (Some(3_000_000), Some(500_000)));
    }

    #[test]
    fn blob_parses_locale() {
        let raw = "@OS\nUbuntu 22.04\n5.15.0\n@LOCALE\nko_KR.UTF-8\n";
        let (s, _) = parse_blob(raw, None, 3);
        assert_eq!(s.os.as_deref(), Some("Ubuntu 22.04"));
        assert_eq!(s.kernel.as_deref(), Some("5.15.0"));
        assert_eq!(s.locale.as_deref(), Some("ko_KR.UTF-8"));
        // 로캘 마커 없으면 None.
        let (s2, _) = parse_blob("@OS\nDebian\n6.1\n", None, 3);
        assert_eq!(s2.locale, None);
    }

    #[test]
    fn blob_net_speed() {
        let mk = |rx: u64, tx: u64| {
            format!("@CPU\ncpu 1 0 1 1\n@NET\n eth0: {rx} 0 0 0 0 0 0 0 {tx} 0\n@DISK\nFilesystem 1024-blocks Used Available\n/d 100 40 60 40% /")
        };
        let (s1, smp) = parse_blob(&mk(1000, 2000), None, 2);
        assert_eq!(s1.net_rx_bps, None); // 첫 표본
        assert_eq!(s1.disk_used_kb, Some(40));
        // 2초 후 rx +4000, tx +1000 → 2000B/s, 500B/s
        let (s2, _) = parse_blob(&mk(5000, 3000), Some(smp), 2);
        assert_eq!((s2.net_rx_bps, s2.net_tx_bps), (Some(2000), Some(500)));
    }
}
