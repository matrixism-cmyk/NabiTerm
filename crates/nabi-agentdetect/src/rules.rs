//! 규칙 타입과 단일 규칙 평가.

use serde::Deserialize;

/// 감지된 에이전트 상태. `Done`은 감지가 아니라 앱의 전이 규칙(working→비포커스 종료)이
/// 만든다 — 화면만 봐서는 "끝났는데 아직 안 봤다"를 알 수 없기 때문.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

impl AgentState {
    pub fn parse(s: &str) -> Self {
        match s {
            "idle" => Self::Idle,
            "working" => Self::Working,
            "blocked" => Self::Blocked,
            "done" => Self::Done,
            _ => Self::Unknown,
        }
    }
}

/// 규칙이 보는 화면 영역.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    /// 화면 아래쪽 몇 줄(TUI 상태·프롬프트 위치).
    #[default]
    Bottom,
    /// OSC 창 제목.
    Title,
}

/// TOML 한 규칙. `regex`/`contains` 중 하나라도 맞고 `not`에 안 걸리면 매치.
#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub id: String,
    /// "idle" | "working" | "blocked" — done/unknown은 규칙으로 만들지 않는다.
    pub state: String,
    /// 높을수록 우선. 같은 값이면 파일 순서.
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub region: Region,
    /// 정규식 목록(하나라도 매치). 컴파일 실패 규칙은 로드 시 버린다.
    #[serde(default)]
    pub regex: Vec<String>,
    /// 부분 문자열 목록(하나라도 포함).
    #[serde(default)]
    pub contains: Vec<String>,
    /// 이 중 하나라도 포함되면 매치 취소(오탐 배제).
    #[serde(default)]
    pub not: Vec<String>,
}

/// 컴파일된 규칙(정규식 사전 컴파일 — 프레임마다 평가해도 싸게).
pub(crate) struct Compiled {
    pub rule: Rule,
    pub state: AgentState,
    pub regex: Vec<regex::Regex>,
}

impl Compiled {
    /// 규칙을 컴파일한다. 정규식이 하나라도 깨져 있으면 None(규칙 통째로 무시).
    pub fn new(rule: Rule) -> Option<Self> {
        let state = AgentState::parse(&rule.state);
        if matches!(state, AgentState::Unknown | AgentState::Done) {
            return None; // 규칙이 만들 수 없는 상태 — 파일 오류로 취급.
        }
        let regex: Option<Vec<_>> = rule.regex.iter().map(|p| regex::Regex::new(p).ok()).collect();
        Some(Self { state, regex: regex?, rule })
    }

    /// 주어진 영역 텍스트에 이 규칙이 매치하는가.
    pub fn matches(&self, text: &str) -> bool {
        if self.rule.not.iter().any(|n| text.contains(n.as_str())) {
            return false;
        }
        self.regex.iter().any(|r| r.is_match(text))
            || self.rule.contains.iter().any(|c| text.contains(c.as_str()))
    }
}
