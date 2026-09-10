//! **색만으로 전달하던 것에 기호를 더한다.**
//!
//! 화면 곳곳에서 색 하나가 뜻을 나른다 — 서버 통계가 붉으면 위험, 세션 아이콘이
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

/// 지금 연결돼 있다(사이드바에서 종류 아이콘을 초록으로 칠하던 자리).
pub(crate) const LIVE: &str = "\u{25cf}";

// ⚠️ 여기 이렇게 적혀 있었다: "색만으로 전달하던 곳은 서버 통계 경고 하나뿐이었다."
// **틀렸다.** 바로 이 파일 첫머리가 "세션 아이콘이 초록이면 연결됨"을 문제의 예로 들면서,
// 정작 고친 것은 상태 표시줄 한 곳뿐이었다(2026-09-10에 세어 보고 알았다).
//
// 실제로 색만 쓰던 자리가 사이드바에 둘 더 있었다 — 연결 표시(초록 아이콘)와
// **표식 띠**(운영/스테이징/개발). 표식은 특히 나쁘다: "이건 운영 서버다"라는 안전
// 신호를 색으로만 말하고 있었다. 같은 표식을 상태 표시줄·우클릭 메뉴·확인 창은
// 글자와 함께 보여 주는데 목록만 아니었다.
//
// 교훈: **"훑어보니 없더라"를 근거로 적지 말 것.** 세어 보고 적는다.

/// 표식(운영/스테이징/개발)을 글자 하나로. 색 띠 위에 겹쳐 그린다.
///
/// 이름을 다 적기에는 자리가 좁다(행 왼쪽 3px 띠). 한 글자면 띠 안에 들어가고,
/// **색을 못 봐도 셋을 구별할 수 있다** — 그게 이 함수의 전부다.
///
/// 언어를 타지 않는 글자를 쓴다. `word()` 가 돌려주는 식별자(`prod`·`staging`·`dev`)의
/// 첫 글자라, 거르기 칸에 치는 낱말과도 이어진다.
pub(crate) fn tag_letter(t: nabi_session::SessionTag) -> &'static str {
    match t {
        nabi_session::SessionTag::None => "",
        nabi_session::SessionTag::Prod => "P",
        nabi_session::SessionTag::Staging => "S",
        nabi_session::SessionTag::Dev => "D",
        nabi_session::SessionTag::Note => "N",
    }
}

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
    ///
    /// `LIVE` 는 예외다 — 글로 이어 붙이는 것이 아니라 정해진 자리에 그려서 공백이 없다.
    #[test]
    fn the_marks_carry_their_own_spacing() {
        assert!(WARN.ends_with(' '));
    }

    /// **표식마다 다른 글자여야 한다.** 겹치면 색을 못 보는 사람에게는 구별이 없는 것과 같다.
    #[test]
    fn 표식_글자가_겹치지_않는다() {
        use nabi_session::SessionTag::*;
        let real = [Prod, Staging, Dev, Note];
        let letters: Vec<&str> = real.iter().map(|t| super::tag_letter(*t)).collect();
        assert!(letters.iter().all(|l| !l.is_empty()), "빈 글자가 있다: {letters:?}");
        let uniq: std::collections::HashSet<&&str> = letters.iter().collect();
        assert_eq!(uniq.len(), letters.len(), "겹치는 글자가 있다: {letters:?}");
    }

    /// 표식 없음은 아무것도 안 그린다 — 대부분의 세션이 여기다.
    #[test]
    fn 표식이_없으면_글자도_없다() {
        assert_eq!(super::tag_letter(nabi_session::SessionTag::None), "");
    }

    /// 글자는 **낱말의 첫 글자**다 — 거르기 칸에 치는 말과 이어져 있어야 외우기 쉽다.
    #[test]
    fn 글자는_낱말의_첫_글자다() {
        use nabi_session::SessionTag::*;
        for t in [Prod, Staging, Dev, Note] {
            let first = t.word().chars().next().unwrap().to_ascii_uppercase().to_string();
            assert_eq!(super::tag_letter(t), first, "{:?}", t);
        }
    }
}
