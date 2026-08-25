//! **자동 응답** — 정해 둔 프롬프트가 뜨면 정해 둔 답을 보낸다.
//!
//! SecureCRT·MobaXterm이 오래 갖고 있던 기능이다. `Continue? [y/N]`, 배포 스크립트의 정형
//! 질문, AI TUI의 확인 프롬프트처럼 **답이 정해져 있는데 사람이 붙어 있어야 하는** 자리를
//! 없앤다.
//!
//! ## 기존 트리거 규칙을 그대로 쓴다
//!
//! `triggers.rs`에 이미 `패턴 -> 액션` 규칙과 그 설정 화면이 있다. 자동 응답을 위해 목록을
//! 하나 더 만들면 사용자는 트리거를 두 곳에서 관리하게 된다. 그래서 액션 하나
//! (`-> reply:y`)를 더하는 쪽을 골랐다.
//!
//! ## 다만 **보는 곳이 다르다**
//!
//! 알림 트리거는 "새로 생긴 줄"을 본다. 그런데 프롬프트는 대개 **줄바꿈 없이** 커서 앞에
//! 머문다(`Continue? [y/N] ` 뒤에 개행이 없다). 새 줄만 보면 영영 안 잡힌다. 그래서 자동
//! 응답은 **화면 아래쪽 몇 줄**을 본다.
//!
//! ## 이 모듈이 위험한 이유를 먼저 적는다
//!
//! 이것은 원격에 우리가 대신 글자를 보내는 일이다. 잘못 맞으면 엉뚱한 곳에 `y`가 들어가고,
//! 그 결과는 되돌릴 수 없을 수도 있다. 그래서 규칙을 코드로 못 박는다.
//!
//! * **기본은 꺼짐**(`terminal.auto_reply`). 규칙이 있어도 켜야 동작한다.
//! * **연속 발동 상한.** 답이 또 프롬프트를 부르는 되먹임에 걸리면 무한히 쏟아붓게 된다.
//! * **비밀번호로 보이는 프롬프트에는 답하지 않는다.** 규칙이 그렇게 돼 있어도 막는다.
//!   자동 응답으로 비밀번호를 보내는 것은 자격증명 볼트를 둔 이유를 스스로 무너뜨린다.
//! * 보내는 글자는 **ASCII 전용**(기존 규율 — 한글 주입은 AI TUI를 깨뜨린다).

use crate::triggers::Action;

/// 발동을 막은 이유. 왜 안 보냈는지 사용자에게 말할 수 있어야 한다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Blocked {
    /// 비밀번호를 묻는 것으로 보인다.
    LooksLikeSecret,
    /// 같은 규칙이 너무 자주 맞았다.
    TooManyTimes,
    /// 보낼 글자에 ASCII 아닌 것이 섞여 있다.
    NonAscii,
}

/// 한 규칙이 연속으로 발동할 수 있는 최대 횟수.
pub(crate) const MAX_STREAK: u32 = 5;

/// 화면 아래에서 몇 줄을 볼 것인가. 프롬프트는 항상 맨 아래에 있다.
pub(crate) const TAIL_ROWS: usize = 3;

/// **비밀번호를 묻는 프롬프트인가.** 여기 걸리면 어떤 규칙도 답하지 않는다.
///
/// 넉넉하게 잡는다 — 놓쳐서 비밀번호를 흘리는 쪽이, 한 번 안 보내서 사람이 직접 치는
/// 쪽보다 훨씬 나쁘다.
pub(crate) fn looks_like_secret(tail: &str) -> bool {
    let t = tail.to_ascii_lowercase();
    const WORDS: &[&str] = &[
        "password", "passphrase", "passwd", "secret", "token", "otp",
        "verification code", "authentication code", "비밀번호", "암호",
    ];
    WORDS.iter().any(|w| t.contains(w))
}

/// 보낼 글자가 안전한가(ASCII 전용, 비어 있지 않음).
pub(crate) fn sendable(s: &str) -> bool {
    !s.is_empty() && s.is_ascii()
}

/// 화면 끝을 보고 보낼 것을 정한다.
///
/// `rules`는 `triggers::parse_rule`이 만든 것 그대로다 — `Action::Reply`만 여기서 본다.
/// `streak`은 (규칙 번호, 연속 발동 횟수).
pub(crate) fn decide(
    tail: &str,
    rules: &[(String, Action)],
    streak: Option<(usize, u32)>,
) -> Result<Option<(usize, String)>, Blocked> {
    if tail.trim().is_empty() {
        return Ok(None);
    }
    // 비밀번호 물음이면 **어떤 규칙도** 보지 않는다. 규칙보다 이 판정이 먼저다.
    if looks_like_secret(tail) {
        return Err(Blocked::LooksLikeSecret);
    }
    let low = tail.to_lowercase(); // 규칙 패턴은 소문자다(triggers::parse_rule).
    for (i, (pat, act)) in rules.iter().enumerate() {
        let Action::Reply(answer) = act else { continue };
        if pat.is_empty() || !low.contains(pat.as_str()) {
            continue;
        }
        if matches!(streak, Some((n, c)) if n == i && c >= MAX_STREAK) {
            return Err(Blocked::TooManyTimes);
        }
        if !sendable(answer) {
            return Err(Blocked::NonAscii);
        }
        // 답 끝의 `\` 하나는 "개행 붙이지 말라"는 뜻(드물지만 필요하다 — 단일 키 응답).
        let (body, newline) = match answer.strip_suffix('\\') {
            Some(b) => (b, false),
            None => (answer.as_str(), true),
        };
        if !sendable(body) {
            return Err(Blocked::NonAscii);
        }
        let mut out = body.to_string();
        if newline {
            out.push('\r');
        }
        return Ok(Some((i, out)));
    }
    Ok(None)
}
