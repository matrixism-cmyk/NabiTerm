//! DEC private mode 1007(alternate scroll mode) 추적.
//!
//! alacritty_terminal은 이 모드를 모르고 지나친다. 하지만 대체 화면을 쓰지 않고 주 화면을
//! 통째로 다시 그리는 TUI(ratatui 계열 — codex CLI가 그렇다)는 **스크롤백에 아무것도 남기지
//! 않으면서** 마우스 보고도 켜지 않는다. 그런 앱에게 휠은 아무 일도 하지 않는다.
//!
//! xterm은 이 경우를 위해 1007을 둔다: "휠을 커서 키로 바꿔 보내라"는 앱의 명시적 요청이다.
//! 요청이 있을 때만 따르므로 일반 셸에는 영향이 없다(셸은 이 모드를 켜지 않는다).

/// `CSI ? … h|l` 시퀀스를 바이트 스트림에서 골라 1007 상태만 따라간다.
#[derive(Default)]
pub struct AltScroll {
    state: State,
    /// 현재 매개변수 숫자들(`;`로 구분) — `1007;1004` 같이 묶어 보내는 앱이 있다.
    params: Vec<u32>,
    cur: Option<u32>,
    /// `CSI` 바로 뒤에 `?`가 왔는가(사설 모드). 없으면 전혀 다른 명령이다.
    private: bool,
    on: bool,
}

#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    /// ESC를 봤다.
    Esc,
    /// `CSI ?`까지 봤다 — 이제 숫자와 `;`를 모은다.
    Params,
}

impl AltScroll {
    /// 앱이 alternate scroll을 요청했는가.
    pub fn enabled(&self) -> bool {
        self.on
    }

    /// 전체 리셋(RIS) 등으로 모드를 초기화한다.
    pub fn clear(&mut self) {
        self.on = false;
        self.reset_scan();
    }

    fn reset_scan(&mut self) {
        self.state = State::Ground;
        self.params.clear();
        self.cur = None;
        self.private = false;
    }

    /// 바이트 하나를 관찰한다(파서와 병렬 — 스트림을 소비하지 않는다).
    pub fn observe(&mut self, b: u8) {
        match self.state {
            State::Ground => {
                if b == 0x1b {
                    self.state = State::Esc;
                }
            }
            // `CSI` 다음이 `?`인지는 Params의 첫 바이트에서 가린다.
            State::Esc if b == b'[' => {
                self.params.clear();
                self.cur = None;
                self.private = false;
                self.state = State::Params;
            }
            State::Esc => self.state = if b == 0x1b { State::Esc } else { State::Ground },
            State::Params => self.param_byte(b),
        }
    }

    /// `CSI` 이후 바이트 처리. 첫 바이트가 `?`가 아니면 사설 모드가 아니므로 버린다.
    fn param_byte(&mut self, b: u8) {
        match b {
            b'?' if self.params.is_empty() && self.cur.is_none() => self.private = true,
            b'0'..=b'9' => {
                let d = u32::from(b - b'0');
                // 값이 터무니없이 커지지 않게 잘라 둔다(악의적 입력 방어).
                self.cur = Some(self.cur.unwrap_or(0).saturating_mul(10).saturating_add(d).min(99999));
            }
            b';' => {
                self.params.push(self.cur.take().unwrap_or(0));
            }
            b'h' | b'l' => {
                if let Some(v) = self.cur.take() {
                    self.params.push(v);
                }
                if self.private && self.params.contains(&1007) {
                    self.on = b == b'h';
                }
                self.reset_scan();
            }
            _ => {
                self.reset_scan();
                // 시퀀스 도중의 ESC는 새 시퀀스의 시작이다(버려서 다음 걸 놓치면 안 된다).
                if b == 0x1b {
                    self.state = State::Esc;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(a: &mut AltScroll, s: &[u8]) {
        for &b in s {
            a.observe(b);
        }
    }

    #[test]
    fn tracks_enable_and_disable() {
        let mut a = AltScroll::default();
        assert!(!a.enabled());
        feed(&mut a, b"\x1b[?1007h");
        assert!(a.enabled());
        feed(&mut a, b"\x1b[?1007l");
        assert!(!a.enabled());
    }

    /// 여러 모드를 한 번에 보내는 앱이 있다 — 묶여 있어도 찾아내야 한다.
    #[test]
    fn finds_it_among_grouped_params() {
        let mut a = AltScroll::default();
        feed(&mut a, b"\x1b[?1004;1007;2004h");
        assert!(a.enabled());
    }

    /// 남의 모드에 반응하면 안 된다(1000번대가 몰려 있어 헷갈리기 쉽다).
    #[test]
    fn ignores_other_modes() {
        let mut a = AltScroll::default();
        feed(&mut a, b"\x1b[?1049h\x1b[?1006h\x1b[?2004h\x1b[?10071h");
        assert!(!a.enabled());
    }

    /// 사설(`?`) 시퀀스가 아니면 무시 — `CSI 1007 h`는 다른 뜻이다.
    #[test]
    fn only_private_sequences_count() {
        let mut a = AltScroll::default();
        feed(&mut a, b"\x1b[1007h");
        assert!(!a.enabled());
    }

    /// 시퀀스가 청크 경계로 잘려 와도 상태가 이어져야 한다(PTY는 아무 데서나 자른다).
    #[test]
    fn survives_split_chunks() {
        let mut a = AltScroll::default();
        feed(&mut a, b"\x1b[?10");
        feed(&mut a, b"07");
        feed(&mut a, b"h");
        assert!(a.enabled());
    }

    #[test]
    fn reset_clears_it() {
        let mut a = AltScroll::default();
        feed(&mut a, b"\x1b[?1007h");
        a.clear();
        assert!(!a.enabled());
    }
}
