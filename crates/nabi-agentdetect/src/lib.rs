//! AI 에이전트 상태 감지 — 화면 하단 텍스트·창 제목을 TOML 규칙으로 분류한다.
//!
//! statusLine 훅을 설치하지 않은 에이전트도 상태(작업 중/입력 대기/완료/유휴)를 알 수 있게
//! 한다. herdr의 "화면 매니페스트" 방식을 벤치마킹했다: 에이전트별 규칙 파일이 화면의
//! **아래쪽 몇 줄**(TUI는 상태·프롬프트를 항상 하단에 그린다)과 OSC 제목을 정규식으로
//! 분류한다. 훅 보고가 있는 pane에는 쓰지 않는다(진실 소스 이원화 방지 — 호출자 책임).
//!
//! 이 크레이트는 순수하다(텍스트 in → 상태 out). 파일 로드는 [`manifests`]가 하되
//! 네트워크는 절대 만지지 않는다 — 규칙은 릴리스에 동봉하고 사용자 폴더로 덮어쓴다.

mod engine;
mod manifests;
mod rules;

pub use engine::{classify, explain, Screen};
pub use manifests::{builtin, load_dir, Manifest};
pub use rules::{AgentState, Rule};

/// 실행 명령의 첫 토큰에서 에이전트 종류를 알아낸다(감지 규칙 선택용).
///
/// 경로·확장자·인자가 붙어도 basename으로 판정한다. 모르는 명령은 None —
/// 억지로 맞히면 엉뚱한 규칙이 셸 출력을 오판한다.
pub fn agent_kind(cmd: &str) -> Option<&'static str> {
    let first = cmd.split_whitespace().next().unwrap_or("");
    let base = first.rsplit(['/', '\\']).next().unwrap_or(first).to_ascii_lowercase();
    let base = base.trim_end_matches(".exe").trim_end_matches(".cmd").trim_end_matches(".bat");
    match base {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "agy" => Some("agy"),
        "gemini" => Some("gemini"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_kind_from_command_shapes() {
        assert_eq!(agent_kind("claude --continue"), Some("claude"));
        assert_eq!(agent_kind(r"C:\Users\u\AppData\Roaming\npm\codex.cmd resume"), Some("codex"));
        assert_eq!(agent_kind("/usr/local/bin/agy"), Some("agy"));
        assert_eq!(agent_kind("cargo build"), None);
        assert_eq!(agent_kind(""), None);
        // 이름에 포함돼도 basename이 다르면 아니다.
        assert_eq!(agent_kind("claude-helper run"), None);
    }
}
