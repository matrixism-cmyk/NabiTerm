//! AI 명령 바가 **어느 값을 믿을 것인가** — 모델·노력 표기의 우선순위.
//!
//! 그리는 일(`aicmdbar`)과 나눠 둔다. 이건 판단이고 저건 그림이라, 한 파일에 있으면
//! 판단을 고칠 때마다 버튼 배치 코드를 지나가야 한다(2026-09-01 줄 수 한도에서 갈랐다).

/// 바에 적을 값을 고른다 — **확실한 것부터.**
///
/// 1. `status` — CLI 가 상태줄(OSC)로 직접 알려 준 값. 이보다 확실한 것은 없다.
/// 2. `pick` — 우리가 방금 `/model X` 를 **보냈다.** 아직 화면에 안 나타났을 뿐이다.
/// 3. `screen` — 스크롤백에서 "Using …" 같은 줄을 찾은 **짐작**.
/// 4. `remembered` — 지난번에 이 CLI 에서 고른 값.
///
/// **2 와 3 의 차례가 이 함수의 존재 이유다.** 예전에는 짐작이 앞서서, 시작할 때 찍힌
/// 줄이 영원히 이겨 모델을 바꿔도 표기가 그대로였다(2026-09-01 사용자 보고).
///
/// 넷 다 모르면 `None` — **아무것도 안 보여 준다.** 모르는데 아는 척하면 그 CLI 에
/// 있지도 않은 모델 이름이 화면에 남는다(같은 보고의 다른 절반이 그것이었다).
pub(crate) fn resolve(
    status: Option<String>,
    pick: Option<String>,
    screen: Option<String>,
    remembered: Option<String>,
) -> Option<String> {
    status.or(pick).or(screen).or(remembered)
}

#[cfg(test)]
mod tests {
    use super::resolve;

    fn s(x: &str) -> Option<String> {
        Some(x.to_string())
    }

    /// **고른 값이 화면 짐작을 이긴다.** 이 한 줄이 사용자가 겪은 결함이다 —
    /// 모델을 바꿔도 시작할 때 찍힌 줄 때문에 표기가 안 바뀌었다.
    #[test]
    fn what_we_just_sent_beats_what_we_guessed_from_the_screen() {
        assert_eq!(resolve(None, s("sonnet"), s("opus"), None).as_deref(), Some("sonnet"));
    }

    /// CLI 가 직접 알려 준 값은 무엇보다 앞선다.
    #[test]
    fn the_cli_telling_us_wins_over_everything() {
        assert_eq!(resolve(s("haiku"), s("sonnet"), s("opus"), s("fable")).as_deref(), Some("haiku"));
    }

    /// 아무것도 모르면 지난번에 고른 값을 쓰고, 그것도 없으면 아무것도 안 보여 준다.
    #[test]
    fn nothing_known_shows_nothing() {
        assert_eq!(resolve(None, None, None, s("fable")).as_deref(), Some("fable"));
        assert_eq!(resolve(None, None, None, None), None);
    }

    /// 화면 짐작은 고른 값이 없을 때만 쓴다(그래도 기억보다는 새 소식이다).
    #[test]
    fn the_screen_still_helps_when_we_have_not_picked() {
        assert_eq!(resolve(None, None, s("opus"), s("fable")).as_deref(), Some("opus"));
    }
}
