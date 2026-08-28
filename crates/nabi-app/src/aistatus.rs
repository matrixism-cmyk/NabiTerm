//! 셸에서 실행 중인 AI 도구(Claude Code 등) 상태바 표시 빌더 — 순수 함수(단위테스트).
//!
//! 두 경로: ① 도구가 발행한 커스텀 상태(pane_status, OSC/nabi cli) ② 셸 통합 run_cmd 자동 감지.

use std::collections::BTreeMap;
use std::time::Duration;

/// 상태바에 그릴 AI 표시 정보.
pub(crate) struct AiDisplay {
    pub label: String,        // 한 줄 요약(🤖 모델 · 토큰 · 상태 · 경과)
    pub tip: String,          // 호버 툴팁(전체 키:값)
    pub gauge: Option<f32>,   // 컨텍스트 사용률 0..1(tokens=used/limit 발행 시)
}

/// 알려진 AI CLI 명령인가(run_cmd 자동 감지용). 첫 토큰 기준.
pub(crate) fn is_ai_command(cmd: &str) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("");
    // 경로/확장자 제거(C:\..\claude.exe → claude).
    let base = first
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(first)
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    matches!(
        base.as_str(),
        "claude" | "aider" | "codex" | "agy" | "llm" | "goose" | "cursor"
            | "opencode" | "crush" | "ollama" | "sgpt" | "cody"
    ) || cmd.contains("claude ")
}

/// "42k/200k" 같은 사용/한도 → 사용률 0..1. k/m 접미사·공백 허용. 형식이 아니면 None.
pub(crate) fn parse_token_usage(s: &str) -> Option<f32> {
    let (u, l) = s.split_once('/')?;
    let used = parse_count(u)?;
    let limit = parse_count(l)?;
    (limit > 0.0).then_some((used / limit).clamp(0.0, 1.0))
}

/// 비용 문자열 → USD 값. "$1.40" / "1.40 USD" / "1.4" 허용. 형식 아니면 None.
pub(crate) fn parse_cost(s: &str) -> Option<f32> {
    let t = s.trim().trim_start_matches('$').trim();
    // 천 단위 쉼표는 무시하고 앞쪽 숫자(소수 포함)만 — "$1,234.50"=1234.5.
    let num: String = t.chars().filter(|c| *c != ',').take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    num.parse::<f32>().ok()
}

/// 컨텍스트 사용률이 임계(0..1)에 도달했는가. 게이지 없으면 false.
pub(crate) fn context_alert(gauge: Option<f32>, thresh: f32) -> bool {
    gauge.is_some_and(|g| g >= thresh)
}

/// 컨텍스트 사용률 다단계: 0=정상(<80%) · 1=경고(80~95%) · 2=위험(≥95%). 게이지 색상용.
pub(crate) fn context_tier(g: f32) -> u8 {
    if g >= 0.95 {
        2
    } else if g >= 0.8 {
        1
    } else {
        0
    }
}

/// 여러 pane의 AI 상태 집계(비용 대시보드용).
pub(crate) struct AiAgg {
    pub panes: usize,       // AI 상태를 발행 중인 pane 수
    pub total_cost: f32,    // 비용 합($)
    pub max_gauge: f32,     // 최대 컨텍스트 사용률 0..1
}

/// 에이전트 pane 상태: 0=idle(대기) · 1=working(명령 실행 중) · 2=blocked(입력 대기).
/// blocked는 에이전트가 발행한 state 키("blocked"/"waiting"/"input")로 판정. 순수 함수.
pub(crate) fn agent_state(status: &BTreeMap<String, String>, running: bool) -> u8 {
    if let Some(s) = status.get("state") {
        let s = s.to_ascii_lowercase();
        if s.contains("block") || s.contains("wait") || s.contains("input") {
            return 2;
        }
    }
    if running {
        1
    } else {
        0
    }
}

/// pane_status 묶음을 집계한다(빈 상태는 제외). 순수 함수(단위테스트).
pub(crate) fn aggregate<'a>(statuses: impl Iterator<Item = &'a BTreeMap<String, String>>) -> AiAgg {
    let (mut panes, mut total_cost, mut max_gauge) = (0usize, 0.0f32, 0.0f32);
    for m in statuses {
        if m.is_empty() {
            continue;
        }
        panes += 1;
        if let Some(c) = m.get("cost").and_then(|v| parse_cost(v)) {
            total_cost += c;
        }
        if let Some(g) = m.get("tokens").and_then(|t| parse_token_usage(t)) {
            max_gauge = max_gauge.max(g);
        }
    }
    AiAgg { panes, total_cost, max_gauge }
}

/// pane_status 키를 의미 순서(model→tokens→cost→burn→status→기타)로 정렬한 표시 조각.
fn ordered_parts(m: &BTreeMap<String, String>) -> Vec<String> {
    const ORDER: [&str; 5] = ["model", "tokens", "cost", "burn", "status"];
    let mut parts = Vec::new();
    for k in ORDER {
        if let Some(v) = m.get(k) {
            let shown = if k == "cost" {
                match parse_cost(v) { Some(c) => format!("\u{1f4b2}{c:.2}"), None => format!("\u{1f4b2}{v}") }
            } else if k == "burn" { format!("\u{1f525}{v}") } else { v.clone() };
            parts.push(shown);
        }
    }
    // ORDER에 없는 나머지 키도 알파벳 순으로 뒤에 붙인다(드리프트 방지).
    for (k, v) in m {
        if !ORDER.contains(&k.as_str()) {
            parts.push(v.clone());
        }
    }
    parts
}

fn parse_count(s: &str) -> Option<f32> {
    let t = s.trim().to_ascii_lowercase();
    let (num, mul) = if let Some(n) = t.strip_suffix('k') {
        (n, 1_000.0)
    } else if let Some(n) = t.strip_suffix('m') {
        (n, 1_000_000.0)
    } else {
        (t.as_str(), 1.0)
    };
    num.trim().replace(',', "").parse::<f32>().ok().map(|v| v * mul) // 천 단위 쉼표 허용.
}

/// 경과 시간(초) → "3m12s" / "45s" / "1h02m".
fn human_elapsed(secs: u64) -> String {
    crate::statusfmt::human_secs(secs)
}

/// AI 표시를 만든다. pane_status(발행값)가 있으면 우선, 없으면 run_cmd 자동 감지.
pub(crate) fn ai_display(
    pane_status: Option<&BTreeMap<String, String>>,
    run_cmd: Option<&str>,
    elapsed: Option<Duration>,
) -> Option<AiDisplay> {
    let el = elapsed.map(|d| human_elapsed(d.as_secs()));
    // ① 발행된 상태가 있으면 그대로 요약.
    if let Some(m) = pane_status.filter(|m| !m.is_empty()) {
        let mut parts = ordered_parts(m);
        if let Some(e) = &el {
            parts.push(e.clone());
        }
        let gauge = m.get("tokens").and_then(|t| parse_token_usage(t));
        let tip = m.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n");
        return Some(AiDisplay { label: format!("\u{1f916} {}", parts.join(" \u{00b7} ")), tip, gauge });
    }
    // ② 셸 통합으로 AI 명령 실행 중이면 자동 표시.
    let cmd = run_cmd?;
    if !is_ai_command(cmd) {
        return None;
    }
    let name = cmd.split_whitespace().next().unwrap_or(cmd);
    let label = match &el {
        Some(e) => format!("\u{1f916} {name} \u{00b7} {e}"),
        None => format!("\u{1f916} {name}"),
    };
    Some(AiDisplay { label, tip: format!("AI 실행 중: {cmd}"), gauge: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ai_commands() {
        assert!(is_ai_command("claude"));
        assert!(is_ai_command("C:\\Users\\u\\claude.exe --resume"));
        assert!(is_ai_command("aider"));
        assert!(is_ai_command("opencode")); // 추가 AI CLI.
        assert!(is_ai_command("ollama run llama3"));
        assert!(!is_ai_command("vim notes.txt"));
        assert!(!is_ai_command("git status"));
    }

    #[test]
    fn token_usage() {
        assert_eq!(parse_token_usage("42k/200k"), Some(0.21));
        assert_eq!(parse_token_usage("100000/200000"), Some(0.5));
        assert_eq!(parse_token_usage("1.5m/3m"), Some(0.5));
        assert_eq!(parse_token_usage("42,000/200,000"), Some(0.21)); // 천 단위 쉼표.
        assert_eq!(parse_token_usage("nope"), None);
    }

    #[test]
    fn elapsed_fmt() {
        // 모양은 이제 `statusfmt::human_secs` 하나가 정한다(배치 AD).
        assert_eq!(human_elapsed(45), "45s");
        assert_eq!(human_elapsed(3 * 60 + 12), "3m 12s");
        assert_eq!(human_elapsed(3600 + 2 * 60), "1h 02m");
    }

    #[test]
    fn display_from_run_cmd() {
        let d = ai_display(None, Some("claude --resume"), Some(Duration::from_secs(72))).unwrap();
        assert!(d.label.contains("claude") && d.label.contains("1m 12s"));
        assert!(ai_display(None, Some("ls -la"), None).is_none());
    }

    #[test]
    fn display_from_pane_status() {
        let mut m = BTreeMap::new();
        m.insert("model".into(), "opus".into());
        m.insert("tokens".into(), "50k/200k".into());
        let d = ai_display(Some(&m), None, None).unwrap();
        assert_eq!(d.gauge, Some(0.25));
        assert!(d.label.contains("opus"));
    }

    #[test]
    fn cost_parsing() {
        assert_eq!(parse_cost("$1.40"), Some(1.40));
        assert_eq!(parse_cost("1.4 USD"), Some(1.4));
        assert_eq!(parse_cost("0.12"), Some(0.12));
        assert_eq!(parse_cost("$1,234.50"), Some(1234.5)); // 천 단위 쉼표.
        assert_eq!(parse_cost("free"), None);
    }

    #[test]
    fn context_threshold() {
        assert!(context_alert(Some(0.85), 0.8));
        assert!(context_alert(Some(0.8), 0.8));
        assert!(!context_alert(Some(0.5), 0.8));
        assert!(!context_alert(None, 0.8));
    }

    #[test]
    fn agent_states() {
        let mut m = BTreeMap::new();
        assert_eq!(agent_state(&m, false), 0); // idle
        assert_eq!(agent_state(&m, true), 1); // working
        m.insert("state".to_string(), "waiting for input".to_string());
        assert_eq!(agent_state(&m, true), 2); // blocked 우선
        m.insert("state".to_string(), "thinking".to_string());
        assert_eq!(agent_state(&m, true), 1); // 비차단 상태면 working
    }

    #[test]
    fn aggregate_costs() {
        let mut a = BTreeMap::new();
        a.insert("cost".to_string(), "$1.40".to_string());
        a.insert("tokens".to_string(), "100k/200k".to_string());
        let mut b = BTreeMap::new();
        b.insert("cost".to_string(), "$0.60".to_string());
        b.insert("tokens".to_string(), "180k/200k".to_string());
        let empty = BTreeMap::new();
        let agg = aggregate([&a, &b, &empty].into_iter());
        assert_eq!(agg.panes, 2); // 빈 상태 제외
        assert!((agg.total_cost - 2.0).abs() < 1e-4);
        assert!((agg.max_gauge - 0.9).abs() < 1e-4);
    }

    #[test]
    fn context_tiers() {
        assert_eq!(context_tier(0.5), 0);
        assert_eq!(context_tier(0.79), 0);
        assert_eq!(context_tier(0.8), 1);
        assert_eq!(context_tier(0.94), 1);
        assert_eq!(context_tier(0.95), 2);
        assert_eq!(context_tier(1.0), 2);
    }

    #[test]
    fn parts_meaningful_order() {
        // BTreeMap 알파벳 순(burn,cost,model,tokens)이지만 표시는 model→tokens→cost→burn.
        let mut m = BTreeMap::new();
        m.insert("status".into(), "thinking".into());
        m.insert("tokens".into(), "50k/200k".into());
        m.insert("cost".into(), "$1.40".into());
        m.insert("model".into(), "opus".into());
        let p = ordered_parts(&m);
        assert_eq!(p[0], "opus");
        assert_eq!(p[1], "50k/200k");
        assert!(p[2].contains("1.40"));
        assert_eq!(p[3], "thinking");
    }
}
