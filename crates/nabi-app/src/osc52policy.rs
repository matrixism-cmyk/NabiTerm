//! 원격이 로컬 클립보드에 쓰는 것(OSC 52)을 어디까지 허용할지.
//!
//! 지금까지는 **제약도 알림도 없이** 원격이 내 클립보드를 덮어썼다. 터미널에 글자를 쓸 수
//! 있는 쪽이면 누구든 가능하다 — 접속한 서버, 그 서버의 `motd`, 내가 `cat`한 파일까지.
//! 복사해 둔 명령이 조용히 다른 것으로 바뀌면, 그걸 브라우저나 관리 콘솔에 붙여넣는 순간
//! 우리 터미널의 붙여넣기 확인은 아무 도움이 안 된다.
//!
//! 그렇다고 통째로 막을 수도 없다 — SSH 너머 nvim/tmux에서 yank한 걸 로컬 클립보드로
//! 가져오는 건 이 기능의 정당하고 흔한 쓰임이다. 그래서 기본값은 **허용하되 알린다**.
//! xterm은 기본 차단, kitty·alacritty는 통제 수단을 두는 쪽으로 정리돼 있다.

/// 설정값(`terminal.osc52_mode`). u8로 저장한다(기존 `sftp_view`와 같은 방식).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Osc52Mode {
    /// 원격의 클립보드 쓰기를 무시한다.
    Block,
    /// 허용하되 무슨 일이 있었는지 알린다(기본).
    Notify,
    /// 조용히 허용(예전 동작).
    Allow,
}

impl Osc52Mode {
    pub(crate) fn from_u8(v: u8) -> Self {
        match v {
            0 => Osc52Mode::Block,
            2 => Osc52Mode::Allow,
            _ => Osc52Mode::Notify,
        }
    }
}

/// 원격 클립보드 쓰기를 어떻게 처리할지 — (적용할까, 알릴까).
///
/// 순수 함수로 둔 이유는 "차단인데 알림만 뜬다" 같은 조합이 생기지 않게 한곳에서 정하기
/// 위해서다. 차단이면 알림도 띄우지 않는다(원격이 계속 시도하면 알림이 도배된다).
pub(crate) fn decide(mode: Osc52Mode) -> (bool, bool) {
    match mode {
        Osc52Mode::Block => (false, false),
        Osc52Mode::Notify => (true, true),
        Osc52Mode::Allow => (true, false),
    }
}

/// 알림에 보여 줄 미리보기 — 클립보드에 들어간 내용을 **짧게** 보여 준다.
///
/// 통째로 띄우지 않는 이유: 원격이 보낸 것이라 길이도 내용도 통제할 수 없고, 비밀번호처럼
/// 보이면 안 될 것이 화면·스크린샷에 남을 수 있다. 줄바꿈은 기호로 접어 한 줄로 만든다.
pub(crate) fn preview(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { '\u{23ce}' } else { c })
        .collect();
    match flat.chars().count() > max {
        true => flat.chars().take(max).collect::<String>() + "\u{2026}",
        false => flat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_map_from_config_value() {
        assert_eq!(Osc52Mode::from_u8(0), Osc52Mode::Block);
        assert_eq!(Osc52Mode::from_u8(1), Osc52Mode::Notify);
        assert_eq!(Osc52Mode::from_u8(2), Osc52Mode::Allow);
        // 모르는 값은 기본(알림 후 허용)으로 — 설정 파일이 깨져도 안전한 쪽.
        assert_eq!(Osc52Mode::from_u8(9), Osc52Mode::Notify);
    }

    /// 차단이면 알림도 뜨지 않는다 — 원격이 반복 시도하면 알림 도배가 된다.
    #[test]
    fn blocking_is_silent() {
        assert_eq!(decide(Osc52Mode::Block), (false, false));
        assert_eq!(decide(Osc52Mode::Notify), (true, true));
        assert_eq!(decide(Osc52Mode::Allow), (true, false));
    }

    #[test]
    fn preview_is_short_and_single_line() {
        assert_eq!(preview("hi", 10), "hi");
        assert_eq!(preview("a\nb", 10), "a\u{23ce}b", "줄바꿈은 기호로 접는다");
        let long = "x".repeat(50);
        let p = preview(&long, 10);
        assert_eq!(p.chars().count(), 11, "10자 + 말줄임");
        assert!(p.ends_with('\u{2026}'));
    }

    /// 원격이 보낸 내용이 그대로 길게 노출되면 안 된다(스크린샷·어깨너머).
    #[test]
    fn preview_does_not_leak_whole_secret() {
        let secret = "AKIA1234567890ABCDEF_super_secret_value";
        let p = preview(secret, 12);
        assert!(!p.contains("super_secret"), "일부만 보여야 한다: {p}");
    }
}
