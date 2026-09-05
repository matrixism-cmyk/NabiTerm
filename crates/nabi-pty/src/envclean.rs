//! 새 셸에 **남의 표식이 딸려 가지 않게** 한다.
//!
//! ## 무슨 일이 있었나
//!
//! 2026-09-05, 업그레이드를 마치고 되살아난 나비텀의 모든 pane 에서 클로드 코드가 이렇게
//! 말했다 — "Transcript saving is off — inherited CLAUDE_CODE_CHILD_SESSION marker".
//! **대화 기록이 저장되지 않고 있었다.** 사용자가 오래 겪던 "대화가 모조리 날아간다"가
//! 이것이었을 수 있다.
//!
//! 어디서 왔나. 업그레이드는 이렇게 이어진다.
//!
//! ```text
//! (클로드 코드가 도는 pane) → 도우미 → 인스톨러 → 새 나비텀 → 새 나비텀이 여는 모든 pane
//! ```
//!
//! 맨 앞의 pane 에는 `CLAUDE_CODE_CHILD_SESSION=1` 이 있었다. 그 뒤로 프로세스가 이어지며
//! 환경이 그대로 상속됐고, **새 나비텀이 여는 모든 셸**이 그 표식을 물려받았다.
//! 그 표식은 "너는 딸린 세션이니 기록을 남기지 마라"라는 뜻이다.
//!
//! ## 무엇을 하나
//!
//! 터미널이 여는 셸은 **맨 처음 셸**이지 누구에게 딸린 세션이 아니다. 그러니 그런 표식은
//! 떼고 띄운다. 사용자가 pane 안에서 직접 어떤 도구를 실행해 그 도구가 자식에게 표식을
//! 붙이는 것은 그 도구의 일이고, 우리가 관여하지 않는다.
//!
//! 목록을 좁게 잡는다. 모르는 변수를 함부로 지우면 사용자가 일부러 넣어 둔 것까지
//! 없애게 된다 — 여기 적는 것은 **"딸린 프로세스"라는 뜻이 박힌 표식**뿐이다.

use portable_pty::CommandBuilder;

/// 새 셸에서 떼어 내는 표식들.
///
/// 늘리기 전에 한 번 더 생각할 것 — 이 목록은 "우리가 안다고 확신하는 것"만 담는다.
const STRIP: &[&str] = &[
    // 클로드 코드가 자식 세션에 붙인다. 있으면 그 세션은 대화 기록을 남기지 않는다.
    "CLAUDE_CODE_CHILD_SESSION",
];

/// 새로 띄우는 셸에서 남의 표식을 떼어 낸다.
pub(crate) fn scrub(cmd: &mut CommandBuilder) {
    for k in STRIP {
        if std::env::var_os(k).is_some() {
            cmd.env_remove(k);
        }
    }
}

/// 이 이름을 떼어 내는가 — 시험과 설명을 위해 밖으로 낸다.
pub fn is_stripped(name: &str) -> bool {
    STRIP.contains(&name)
}

#[cfg(test)]
mod tests {
    /// 목록은 좁아야 한다. 넓히면 사용자가 넣어 둔 것까지 지운다.
    #[test]
    fn 아는_표식만_뗀다() {
        assert!(super::is_stripped("CLAUDE_CODE_CHILD_SESSION"));
        for keep in ["PATH", "HOME", "NABI_PANE_ID", "CLAUDE_CODE_SSE_PORT", "TERM"] {
            assert!(!super::is_stripped(keep), "{keep} 은 떼면 안 된다");
        }
    }
}
