//! **색만으로 전달하던 것에 기호를 더한다.**
//!
//! 화면 곳곳에서 색 하나가 뜻을 나른다 — 서버 통계가 빨개지면 위험, 세션 아이콘이
//! 초록이면 연결됨. 색을 못 보거나 흐리게 보는 사용자에게 그 뜻은 **전달되지 않는다.**
//!
//! 여기서 하는 일은 단순하다: 켜면 그 자리에 **글자 하나**를 덧붙인다. 끄면 지금 그대로다.
//!
//! ## 왜 기본이 꺼짐인가
//!
//! 기호가 늘 붙으면 상태 표시줄이 길어지고, 색으로 충분히 읽는 사용자에게는 잡음이다.
//! 필요한 사람이 켠다. 다만 **켜는 자리를 찾기 쉽게** 접근성 페이지에 모아 두었다.

/// 켜져 있으면 `mark`를, 아니면 빈 글자.
///
/// 호출부가 `format!("{}{}", cue(on, "⚠ "), text)`처럼 쓰도록 **접두사째** 돌려준다 —
/// 자리마다 `if`를 쓰면 어떤 곳은 붙고 어떤 곳은 빠진다.
pub(crate) fn cue(on: bool, mark: &'static str) -> &'static str {
    match on {
        true => mark,
        false => "",
    }
}

/// 경고(빨강으로 칠하던 자리).
pub(crate) const WARN: &str = "\u{26a0} ";
// 성공 쪽 기호(✓)는 두지 않았다 — 훑어보니 이 프로그램은 성공을 알릴 때 이미 대부분
// 기호를 함께 쓰고 있었다(전송 큐 ✓/✗, 도달 확인 표시, 종료 코드 칩). 색만으로 전달하던
// 곳은 서버 통계 경고 하나뿐이었다. 쓸 자리가 없는 상수를 미리 만들어 두면 다음 사람이
// "왜 안 쓰지?"를 묻게 된다.

#[cfg(test)]
mod tests {
    use super::{cue, WARN};

    /// 꺼져 있으면 **아무것도 붙지 않는다** — 지금 화면이 달라지면 회귀다.
    #[test]
    fn off_changes_nothing() {
        assert_eq!(cue(false, WARN), "");
        assert_eq!(format!("{}{}", cue(false, WARN), "CPU 91%"), "CPU 91%");
    }

    #[test]
    fn on_prefixes_the_mark() {
        assert_eq!(format!("{}{}", cue(true, WARN), "CPU 91%"), "\u{26a0} CPU 91%");
    }

    /// 기호는 **뒤에 공백을 달고 온다** — 호출부가 공백을 각자 붙이면 어긋난다.
    #[test]
    fn the_marks_carry_their_own_spacing() {
        assert!(WARN.ends_with(' '));
    }
}
