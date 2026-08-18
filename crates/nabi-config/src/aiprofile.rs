//! AI 터미널 프로필 설정(세션▸새 AI 터미널) — schema.rs에서 분리(라인 한도).

use serde::{Deserialize, Serialize};

/// AI 터미널 프로필 — 미리 설정한 셸+AI CLI+옵션으로 새 터미널을 여는 동선(세션▸새 AI 터미널).
/// claude/codex 등 CLI마다 붙이는 스위치가 달라, 프로필로 저장해 원클릭 실행한다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiProfileCfg {
    pub name: String,
    /// 셸("powershell"/"pwsh"/"cmd"/"wsl"/"gitbash", 빈 값 = 기본 셸 설정을 따름).
    pub shell: String,
    /// 실행할 CLI 명령(claude/codex/gemini/aider/… 또는 임의 명령).
    pub cmd: String,
    /// CLI 뒤에 붙는 인자들 — 체크박스 프리셋과 직접 입력 모두 여기로 합쳐 저장(SSOT).
    pub args: Vec<String>,
}

impl Default for AiProfileCfg {
    fn default() -> Self {
        Self { name: String::new(), shell: String::new(), cmd: "claude".into(), args: Vec::new() }
    }
}
