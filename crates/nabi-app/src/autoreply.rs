//! **자동 응답 트리거** — 정해 둔 출력이 보이면 정해 둔 답을 보낸다.
//!
//! SecureCRT·MobaXterm이 오래 갖고 있던 기능이고 우리에게는 없었다. `Continue? [y/N]`,
//! 배포 스크립트의 정형 질문, AI TUI가 띄우는 확인 프롬프트처럼 **답이 정해져 있는데
//! 사람이 붙어 있어야 하는** 자리를 없앤다.
//!
//! ## 이 모듈이 위험한 이유를 먼저 적는다
//!
//! 이것은 **원격에 우리가 대신 글자를 보내는 일**이다. 잘못 맞으면 엉뚱한 곳에 `y`가
//! 들어가고, 그 결과는 되돌릴 수 없을 수도 있다. 그래서 규칙을 코드로 못 박는다.
//!
//! * **기본은 꺼짐.** 규칙이 있어도 세션에서 켜야 동작한다.
//! * **연속 발동 상한.** 같은 규칙이 계속 맞으면 멈춘다 — 답이 다시 프롬프트를 부르는
//!   되먹임에 걸리면 무한히 입력을 쏟아붓게 된다.
//! * **비밀번호로 보이는 프롬프트에는 답하지 않는다.** 자동 응답으로 비밀번호를 보내는 것은
//!   자격증명 볼트를 둔 이유를 스스로 무너뜨리는 짓이다. 규칙이 그렇게 설정돼 있어도 막는다.
//! * 보내는 글자는 **ASCII 전용**(기존 규율 — 한글을 주입하면 AI TUI가 깨진다).
//!
//! 매칭은 전부 순수 함수다. 서버 없이 시험할 수 있고, 그래야 한다.

/// 규칙 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rule {
    /// 화면 끝에서 이 글이 보이면 발동.
    pub when: String,
    /// 보낼 글자(개행은 `send_newline`이 붙인다).
    pub send: String,
    /// 정규식으로 볼 것인가.
    pub regex: bool,
    /// 보낸 뒤 Enter를 붙일 것인가.
    pub send_newline: bool,
    /// 이 규칙을 쓸 것인가.
    pub enabled: bool,
}

impl Default for Rule {
    fn default() -> Self {
        Self { when: String::new(), send: String::new(), regex: false, send_newline: true, enabled: true }
    }
}

/// 발동을 막는 이유. 왜 안 보냈는지 사용자에게 말해 줄 수 있어야 한다.
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
///
/// 답이 또 프롬프트를 부르는 되먹임(잘못 만든 규칙에서 흔하다)에서 무한히 쏟아붓지 않게.
pub(crate) const MAX_STREAK: u32 = 5;

/// 화면 끝 몇 글자를 보고 판단하는가. 프롬프트는 항상 끝에 있다.
const TAIL: usize = 400;

/// **비밀번호를 묻는 프롬프트인가.** 여기 걸리면 어떤 규칙도 답하지 않는다.
///
/// 넉넉하게 잡는다 — 놓쳐서 비밀번호를 자동으로 흘리는 쪽이, 한 번 안 보내서 사람이 직접
/// 치는 쪽보다 훨씬 나쁘다.
pub(crate) fn looks_like_secret(tail: &str) -> bool {
    let t = tail.to_ascii_lowercase();
    const WORDS: &[&str] = &[
        "password", "passphrase", "passwd", "secret", "token", "otp",
        "verification code", "authentication code", "비밀번호", "암호",
    ];
    WORDS.iter().any(|w| t.contains(w))
}

/// 보낼 글자가 안전한가(ASCII 전용).
pub(crate) fn sendable(s: &str) -> bool {
    !s.is_empty() && s.is_ascii()
}

/// 화면 끝에서 발동할 규칙을 찾는다.
///
/// `streak`은 (규칙 번호, 연속 발동 횟수)로, 같은 규칙이 계속 맞을 때 멈추기 위한 것이다.
/// 돌려주는 것은 `(규칙 번호, 보낼 바이트)` 또는 막힌 이유.
pub(crate) fn decide(
    screen_tail: &str,
    rules: &[Rule],
    streak: Option<(usize, u32)>,
) -> Result<Option<(usize, String)>, Blocked> {
    let tail = tail_of(screen_tail);
    if tail.trim().is_empty() {
        return Ok(None);
    }
    // 비밀번호 물음이면 **어떤 규칙도** 보지 않는다. 규칙보다 이 판정이 먼저다.
    if looks_like_secret(tail) {
        return Err(Blocked::LooksLikeSecret);
    }
    for (i, r) in rules.iter().enumerate() {
        if !r.enabled || r.when.is_empty() || !matches(tail, r) {
            continue;
        }
        if matches!(streak, Some((n, c)) if n == i && c >= MAX_STREAK) {
            return Err(Blocked::TooManyTimes);
        }
        if !sendable(&r.send) {
            return Err(Blocked::NonAscii);
        }
        let mut out = r.send.clone();
        if r.send_newline {
            out.push('\r');
        }
        return Ok(Some((i, out)));
    }
    Ok(None)
}

/// 화면의 마지막 부분만 본다 — 프롬프트는 끝에 있고, 위쪽 옛 글에 걸리면 안 된다.
fn tail_of(s: &str) -> &str {
    match s.char_indices().nth_back(TAIL.saturating_sub(1)) {
        Some((i, _)) => &s[i..],
        None => s,
    }
}

/// 규칙이 이 글에 맞는가.
fn matches(tail: &str, r: &Rule) -> bool {
    if !r.regex {
        return tail.contains(&r.when);
    }
    // 잘못된 정규식은 **맞지 않는 것으로** 본다. 여기서 터지면 pane이 죽는다.
    match regex::Regex::new(&r.when) {
        Ok(re) => re.is_match(tail),
        Err(_) => false,
    }
}
