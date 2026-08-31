//! `agentguide` 시험 — 설명서와 실제 동사 목록이 어긋나지 않는지 본다.
//!
//! 본문에서 떼어 낸 까닭은 줄 수다(설명서 자체가 400줄 가까이 된다).
//! `trail_tests.rs` 와 같은 방식이다.

/// 동사를 파는 곳이 세 파일에 나뉘어 있다 — 하나만 보면 있는 것을 없다고 한다.
fn verb_sources() -> String {
    [
        include_str!("../../nabi-control/src/clientverbs.rs"),
        include_str!("../../nabi-control/src/client.rs"),
        include_str!("../../nabi-control/src/clientagent.rs"),
    ]
    .concat()
}

/// 소스에서 실제로 파는 낱말을 모두 모은다.
///
/// 두 갈래다. 대부분은 `Some("x")` 로 하나씩 파지만, 웹 조종처럼 **배열에 적어 두고
/// 접두어를 붙여** 파는 것도 있다. 한쪽만 보면 있는 것을 없다고 한다 — 실제로 그렇게
/// 걸렸다. 손으로 적지 않고 두 갈래를 다 읽는다.
fn known_verbs() -> Vec<String> {
    let src = verb_sources();
    let mut out: Vec<String> = src
        .split("Some(\"")
        .skip(1)
        .filter_map(|p| p.split('"').next().map(str::to_string))
        .filter(|w| !w.is_empty() && !w.starts_with("--"))
        .collect();
    out.extend(prefixed_verbs(&src));
    out
}

/// `const ACTS: [&str; N] = ["back", ...]` + `strip_prefix("web-")` 꼴을 펴 낸다.
///
/// 배열과 접두어를 **소스에서 읽는다.** 여기 목록을 또 적으면 언젠가 어긋나고,
/// 어긋난 검사기는 검사하지 않는 것보다 나쁘다.
fn prefixed_verbs(src: &str) -> Vec<String> {
    let Some(arr) = src.split("const ACTS: [&str;").nth(1) else {
        return Vec::new();
    };
    let Some(items) = arr.split('[').nth(1).and_then(|s| s.split(']').next()) else {
        return Vec::new();
    };
    let prefix = src
        .split("strip_prefix(\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("");
    items
        .split(',')
        .filter_map(|w| w.trim().trim_matches('"').split('"').next())
        .filter(|w| !w.is_empty())
        .map(|w| format!("{prefix}{w}"))
        .collect()
}

/// 설명서에 적힌 동사가 **실제로 있는 동사인지** 대조한다.
///
/// 이 설명서는 AI 에게 주는 것이다. 없는 동사를 적어 두면 AI 가 그것을 부르고 실패한다.
/// 그리고 실패한 AI 는 우리 프로그램이 고장 났다고 판단한다.
///
/// 손으로 관리하는 목록은 언젠가 실제와 달라진다 — 설정 검색 색인에서 이미 두 번 겪었다.
#[test]
fn every_verb_in_the_guide_really_exists() {
    let known = known_verbs();
    let mut missing = Vec::new();
    for (_, rest) in verb_lines() {
        let Some(verb) = rest.split([' ', '`']).find(|w| !w.is_empty()) else { continue };
        if !known.iter().any(|k| k == verb) {
            missing.push(verb.to_string());
        }
    }
    assert!(missing.is_empty(), "설명서에만 있고 실제로는 없는 동사: {missing:?}");
}

/// 새로 만든 동사를 설명서에 적는 것을 잊지 않게 한다.
///
/// 앞의 시험과 방향이 반대다. 그쪽은 "없는 것을 적었나", 이쪽은 "있는 것을 빠뜨렸나"를 본다.
/// 둘 다 있어야 목록이 실제와 같아진다.
#[test]
fn every_real_verb_is_written_down() {
    // **낱말이 산문에 섞여 있는 것으로는 안 된다.** 예전에는 `guide.contains(v)` 였는데,
    // `restart` 를 새로 만들었을 때 설명서에 없는데도 통과했다 — 다른 항목 설명에
    // "restarts nabiTerm" 이라고 적혀 있었기 때문이다(2026-08-31에 겪었다).
    //
    // 그래서 **적힌 자리의 모양**을 본다. 설명서는 동사를 늘 `- \`nabi cli <낱말>` 로
    // 시작하는 줄에 적는다 — 앞 시험(`every_verb_in_the_guide_really_exists`)이 같은
    // 모양을 읽으므로, 둘이 같은 규칙을 본다.
    let documented: Vec<String> = verb_lines()
        .into_iter()
        .filter_map(|(_, r)| r.split([' ', '`']).find(|w| !w.is_empty()))
        .map(str::to_string)
        .collect();
    // 두 낱말짜리 동사(`agent report` 처럼)는 앞낱말만 적히므로 그것도 받아 준다.
    let mut absent: Vec<String> = known_verbs()
        .into_iter()
        .filter(|v| !documented.iter().any(|d| d == v))
        .filter(|v| !SUBWORDS.contains(&v.as_str()))
        .collect();
    absent.sort();
    absent.dedup();
    assert!(absent.is_empty(), "실제로 있는데 설명서에 없는 동사: {absent:?}");
}

/// 동사가 적힌 줄들 — `(등급, 동사부터의 나머지)`. 세 시험이 같은 규칙으로 읽는다.
///
/// 설명서의 모양은 `- (act) \`nabi cli web-list\` — 설명` 이다. 등급을 낱말 **앞**에 두는
/// 까닭은 줄 끝이 이미 설명으로 차 있어서다.
fn verb_lines() -> Vec<(&'static str, &'static str)> {
    crate::agentguide::AGENT_GUIDE_MD
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("- ("))
        .filter_map(|r| r.split_once(") `nabi cli "))
        .collect()
}

/// 등급 표시를 빠뜨린 줄이 없는가 — 빠지면 위 시험이 그 줄을 아예 안 본다.
#[test]
fn no_verb_line_is_missing_its_tier() {
    let bare: Vec<&str> = crate::agentguide::AGENT_GUIDE_MD
        .lines()
        .filter(|l| l.trim_start().starts_with("- `nabi cli "))
        .collect();
    assert!(bare.is_empty(), "등급 표시가 없는 줄: {bare:?}");
}

/// 두 낱말짜리 동사의 **뒷낱말** — 설명서에는 앞낱말 줄에 함께 적힌다.
const SUBWORDS: &[&str] = &[
    "report", "release", "session", "prompt", "explain", "wait", "install", "create",
    "export", "apply", "set", "clear", "audit", "schema", "goto", "zoom", "pdf", "shot",
    "eval",
];
