//! 매니페스트 평가 — 화면 스냅샷에 규칙을 우선순위대로 적용한다.

use crate::manifests::Manifest;
use crate::rules::{AgentState, Region};

/// 평가 입력: pane 화면의 스냅샷(호출자가 만들어 준다 — 이 크레이트는 I/O가 없다).
pub struct Screen<'a> {
    /// 화면 아래쪽 몇 줄의 평문(권장 3~5줄 — TUI 상태 표시 위치).
    pub bottom: &'a str,
    /// OSC 창 제목.
    pub title: &'a str,
}

/// 화면을 분류한다. 반환=(상태, 매치된 규칙 id). 아무 규칙도 안 맞으면 (Unknown, None).
///
/// blocked 오탐이 제일 해롭다(사용자를 헛걸음시킨다) — 그래서 규칙 파일 쪽에서 blocked는
/// "실제로 보이는 승인/질문 UI"만 잡도록 보수적으로 쓴다(herdr과 같은 원칙).
pub fn classify<'m>(m: &'m Manifest, s: &Screen) -> (AgentState, Option<&'m str>) {
    // 로드 시 priority 내림차순 정렬돼 있다 — 첫 매치가 답.
    for c in &m.rules {
        let text = match c.rule.region {
            Region::Bottom => s.bottom,
            Region::Title => s.title,
        };
        if c.matches(text) {
            return (c.state, Some(c.rule.id.as_str()));
        }
    }
    (AgentState::Unknown, None)
}

/// 상태 판정 근거를 사람이 읽을 문장으로 — `nabi cli agent explain`(A4)용.
pub fn explain(m: &Manifest, s: &Screen) -> String {
    let (state, rule) = classify(m, s);
    match rule {
        Some(id) => format!("{:?} — 규칙 '{id}' 매치(매니페스트 '{}')", state, m.id),
        None => format!("{:?} — 매치된 규칙 없음(매니페스트 '{}', 규칙 {}개)", state, m.id, m.rules.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifests::Manifest;

    fn manifest(toml: &str) -> Manifest {
        Manifest::parse(toml).expect("test manifest")
    }

    const M: &str = r#"
id = "t"
[[rules]]
id = "working_esc"
state = "working"
priority = 100
contains = ["esc to interrupt"]

[[rules]]
id = "blocked_prompt"
state = "blocked"
priority = 200
contains = ["Do you want"]

[[rules]]
id = "idle_hint"
state = "idle"
priority = 10
contains = ["? for shortcuts"]
"#;

    #[test]
    fn priority_orders_evaluation() {
        let m = manifest(M);
        // working과 blocked 텍스트가 동시에 보이면 priority 높은 blocked가 이긴다.
        let s = Screen { bottom: "Do you want to run this? esc to interrupt", title: "" };
        assert_eq!(classify(&m, &s), (AgentState::Blocked, Some("blocked_prompt")));
    }

    #[test]
    fn regions_are_independent() {
        let m = manifest(
            r#"
id = "t"
[[rules]]
id = "title_spinner"
state = "working"
region = "title"
regex = ['^[\x{2800}-\x{28FF}\x{25D0}-\x{25D3}\x{2733}] ']
"#,
        );
        let s = Screen { bottom: "", title: "\u{2837} building" };
        assert_eq!(classify(&m, &s).0, AgentState::Working);
        // 같은 글자가 bottom에 있어도 title 규칙은 안 잡는다.
        let s = Screen { bottom: "\u{2837} building", title: "shell" };
        assert_eq!(classify(&m, &s).0, AgentState::Unknown);
    }

    #[test]
    fn not_clause_cancels_match() {
        let m = manifest(
            r#"
id = "t"
[[rules]]
id = "w"
state = "working"
contains = ["esc to interrupt"]
not = ["transcript"]
"#,
        );
        let s = Screen { bottom: "transcript view - esc to interrupt", title: "" };
        assert_eq!(classify(&m, &s).0, AgentState::Unknown);
    }

    #[test]
    fn no_match_is_unknown_not_idle() {
        let m = manifest(M);
        let s = Screen { bottom: "some random shell output", title: "" };
        assert_eq!(classify(&m, &s), (AgentState::Unknown, None));
    }

    #[test]
    fn explain_names_the_rule() {
        let m = manifest(M);
        let s = Screen { bottom: "... esc to interrupt", title: "" };
        assert!(explain(&m, &s).contains("working_esc"));
    }
}
