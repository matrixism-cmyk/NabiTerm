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
    // 껍데기(`npx`·`sudo`·`wsl`…)를 벗기는 일은 `cmdbase` 한 곳에만 있다 — 예전에는
    // 여기와 `aihandoff` 가 각자 첫 토막만 잘라 보다가 **둘 다** 감싼 실행을 놓쳤다.
    let Some(base) = crate::cmdbase::real_command_base(cmd) else { return false };
    matches!(
        base.as_str(),
        "claude" | "claude-code" | "aider" | "codex" | "agy" | "gemini" | "llm" | "goose"
            | "cursor" | "warp" | "opencode" | "crush" | "ollama" | "sgpt" | "cody"
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
    // `task` 는 **지금 무엇을 하고 있는가**다. 모델 이름 다음에 둔다 — 이 줄을 볼 때
    // 사람이 가장 먼저 궁금해하는 것이 "지금 뭐 하는 중이지?"다(사용자 요청 2026-08-29).
    const ORDER: [&str; 6] = ["model", "task", "tokens", "cost", "burn", "status"];
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
    progress: Option<u8>,
) -> Option<AiDisplay> {
    let el = elapsed.map(|d| human_elapsed(d.as_secs()));
    // 진행률은 흐른 시간보다 앞에 둔다 — "얼마나 됐나"보다 "얼마나 남았나"가 궁금하다.
    let pg = progress.map(|p| format!("\u{23f3}{p}%"));
    // ① 발행된 상태가 있으면 그대로 요약.
    if let Some(m) = pane_status.filter(|m| !m.is_empty()) {
        let mut parts = ordered_parts(m);
        if let Some(p) = &pg {
            parts.push(p.clone());
        }
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
    let extra: Vec<String> = [pg, el].into_iter().flatten().collect();
    let label = match extra.is_empty() {
        true => format!("\u{1f916} {name}"),
        false => format!("\u{1f916} {name} \u{00b7} {}", extra.join(" \u{00b7} ")),
    };
    Some(AiDisplay { label, tip: format!("AI 실행 중: {cmd}"), gauge: None })
}

#[cfg(test)]
#[path = "aistatus_tests.rs"]
mod tests;
