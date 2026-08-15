//! 매니페스트(에이전트별 규칙 묶음) 로드 — 내장 + 사용자 폴더 오버라이드.

use crate::rules::{Compiled, Rule};
use serde::Deserialize;

/// TOML 파일 하나 = 에이전트 하나의 규칙 묶음.
#[derive(Deserialize)]
struct RawManifest {
    id: String,
    #[serde(default)]
    rules: Vec<Rule>,
}

pub struct Manifest {
    pub id: String,
    pub(crate) rules: Vec<Compiled>,
}

impl Manifest {
    /// TOML 텍스트를 파싱·컴파일한다. 깨진 정규식 규칙은 조용히 버린다(전체 실패 방지).
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawManifest = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut rules: Vec<Compiled> = raw.rules.into_iter().filter_map(Compiled::new).collect();
        // 우선순위 내림차순 — classify는 첫 매치를 취한다.
        rules.sort_by_key(|c| -c.rule.priority);
        Ok(Self { id: raw.id, rules })
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// 릴리스에 동봉하는 내장 규칙. 네트워크 갱신은 하지 않는다 — 폐쇄망 정체성.
/// 사용자 폴더(load_dir)가 같은 id를 내면 그쪽이 이긴다.
pub fn builtin() -> Vec<Manifest> {
    [include_str!("../rules/claude.toml"), include_str!("../rules/codex.toml")]
        .iter()
        .filter_map(|t| Manifest::parse(t).ok())
        .collect()
}

/// 사용자 오버라이드 폴더(`*.toml`)를 읽는다. 없거나 비면 빈 Vec.
pub fn load_dir(dir: &std::path::Path) -> Vec<Manifest> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "toml") {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(m) = Manifest::parse(&text) {
                    out.push(m);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{classify, Screen};
    use crate::rules::AgentState;

    /// 내장 매니페스트는 항상 파싱돼야 한다(릴리스 게이트).
    #[test]
    fn builtin_manifests_parse() {
        let all = builtin();
        assert_eq!(all.len(), 2, "claude + codex");
        for m in &all {
            assert!(m.rule_count() > 0, "{} 규칙이 비었다", m.id);
        }
    }

    /// 실측 화면 조각으로 내장 규칙을 검증한다(실제 pane에서 캡처한 문구).
    #[test]
    fn builtin_rules_match_real_screens() {
        let all = builtin();
        let claude = all.iter().find(|m| m.id == "claude").unwrap();
        let codex = all.iter().find(|m| m.id == "codex").unwrap();

        // Claude Code: 작업 중에는 하단에 "esc to interrupt"가 뜬다.
        let s = Screen { bottom: "\u{2733} Compacting\u{2026} (esc to interrupt)", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Working);
        // 유휴 하단 힌트.
        let s = Screen { bottom: "\u{23f8} manual mode on \u{b7} ? for shortcuts", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Idle);
        // 권한 확인 다이얼로그 = blocked.
        let s = Screen { bottom: "Do you want to proceed?\n \u{276f} 1. Yes", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Blocked);
        // 세션 재개 다이얼로그(실측 pane 1): 하단은 확인 힌트만 보인다.
        let s = Screen { bottom: "  3. Don't ask me again\n\n  Enter to confirm \u{b7} Esc to cancel", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Blocked);
        // bypass 모드 유휴 하단(실측 pane 3 footer에서 working 마커를 뺀 형태).
        let s = Screen { bottom: "  \u{23f5}\u{23f5} bypass permissions on (shift+tab to cycle)", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Idle);
        // 같은 footer라도 working 마커가 있으면 working이 이긴다(실측 pane 3 원문).
        let s = Screen { bottom: "  \u{23f5}\u{23f5} bypass permissions on (shift+tab to cycle) \u{b7} esc to interrupt", title: "" };
        assert_eq!(classify(claude, &s).0, AgentState::Working);
        // codex 유휴 컴포저(실측 pane 2)는 규칙 없음 → Unknown(앱이 running 여부로 폴백).
        let s = Screen { bottom: "\u{203a} Implement {feature}\n\n  gpt-5.6-sol default \u{b7} ~", title: "" };
        assert_eq!(classify(codex, &s).0, AgentState::Unknown);

        // codex: 작업 중 "(0s • esc to interrupt)" — 실측(modeprobe 캡처).
        let s = Screen { bottom: "Booting MCP server (0s \u{2022} esc to interrupt)", title: "" };
        assert_eq!(classify(codex, &s).0, AgentState::Working);
        // 신뢰/승인 프롬프트 = blocked — 실측.
        let s = Screen { bottom: "\u{203a} 1. Yes, continue\n Press enter to continue", title: "" };
        assert_eq!(classify(codex, &s).0, AgentState::Blocked);
        // 전사 오버레이는 상태 판정에서 제외(not 절) — 열어 둔 채 idle일 수 있다.
        let s = Screen { bottom: "q to quit esc to edit prev (esc to interrupt)", title: "" };
        assert_eq!(classify(codex, &s).0, AgentState::Unknown);
    }

    #[test]
    fn user_dir_overrides_load() {
        let dir = std::env::temp_dir().join(format!("nabi-adm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.toml"), "id='x'\n[[rules]]\nid='r'\nstate='working'\ncontains=['GO']\n").unwrap();
        std::fs::write(dir.join("broken.toml"), "not toml [").unwrap(); // 깨진 파일은 무시.
        let ms = load_dir(&dir);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].id, "x");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
