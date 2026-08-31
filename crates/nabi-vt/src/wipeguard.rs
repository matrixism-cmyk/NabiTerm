//! **앱이 "지나간 내용을 지워라"라고 할 때** 그것을 세고, 원하면 막는다.
//!
//! ## 무슨 일이 있었나
//!
//! `CSI 3 J`(xterm 의 ED 3)는 화면이 아니라 **스크롤백을 지운다.** 화면을 덮어 그리는
//! TUI 가 새로 그리기 전에 이것을 보내는 일이 잦은데, 그러면 사람이 위로 올려 보려던
//! 지나간 내용이 **그 순간 사라진다.** 사용자에게는 "나비텀이 기록을 잃어버렸다"로 보인다.
//!
//! 2026-08-31에 그 보고를 받고 재 봤더니, 화면을 덮어 그리는 프로그램은 800줄을 찍고도
//! 스크롤백에 0줄을 남겼다. 지우기까지 겹치면 남을 것이 더 없다.
//!
//! ## 왜 막는 쪽을 기본으로 두는가
//!
//! 지운 것은 되돌릴 수 없고, 안 지운 것은 언제든 지울 수 있다(메뉴의 "스크롤백 비우기").
//! 막아서 손해 보는 것은 `clear` 를 쳤을 때 위쪽 기록이 남아 있는 것뿐인데, 스크롤백은
//! 원래 그러라고 있는 자리다. kitty 도 앱이 보내는 이 시퀀스를 그대로 따르지 않는다.
//!
//! ## 왜 바이트를 걸러 내는가
//!
//! 파서에 넣고 나면 이미 지워진 뒤다. 그래서 **넣기 전에** 그 시퀀스만 빼낸다.
//! PTY 는 아무 데서나 자르므로 시퀀스가 청크 두 개에 걸칠 수 있다 — 진행 중인 조각은
//! 들고 있다가 다음 청크와 이어 본다.

/// 이 설정은 사람이 한 번 정하면 **모든 pane 에 같이** 걸린다.
///
/// pane 마다 들고 다니면 스폰 경로 다섯 군데에 값을 흘려야 하고, 설정을 바꿔도 **이미 열린
/// pane 은 그대로**가 된다. 사용자에게는 "껐는데 안 꺼진다"로 보인다. 그래서 전역에 둔다.
static PROTECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// 설정이 바뀌면 여기에 넣는다 — 열려 있는 pane 에도 다음 출력부터 곧바로 적용된다.
pub fn set_default_protect(on: bool) {
    PROTECT.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// 지금 기본값.
pub fn default_protect() -> bool {
    PROTECT.load(std::sync::atomic::Ordering::Relaxed)
}

/// 스크롤백을 지우라는 시퀀스를 골라내는 상태 기계.
#[derive(Default)]
pub struct WipeGuard {
    state: State,
    /// 아직 판단이 끝나지 않아 들고 있는 바이트(`ESC [ 3` 까지 온 상태 등).
    pending: Vec<u8>,
    /// 지금까지 모은 매개변수들.
    params: Vec<u32>,
    cur: Option<u32>,
    /// `CSI` 바로 뒤에 `?` 가 왔는가 — 사설 모드는 ED 가 아니다.
    private: bool,
    /// 앱이 스크롤백을 지우려 한 횟수(막았든 안 막았든 센다).
    wipes: u32,
}

#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Esc,
    Params,
}

/// 들고 있을 수 있는 최대 길이. 이보다 길어지면 시퀀스가 아니라고 보고 흘려보낸다.
///
/// 끝나지 않는 조각을 영원히 붙들면 화면이 멈춘 것처럼 보인다. 실제 ED 시퀀스는
/// 길어야 열 바이트 남짓이다.
const MAX_PENDING: usize = 32;

impl WipeGuard {
    /// 앱이 스크롤백을 지우려 한 횟수.
    pub fn wipes(&self) -> u32 {
        self.wipes
    }

    /// 바이트 열에서 `CSI … 3 … J` 를 골라낸다.
    ///
    /// `protect` 가 참이면 그 시퀀스를 **빼고** 돌려준다(파서가 못 본다). 거짓이면 세기만
    /// 하고 그대로 흘려보낸다 — 세는 것과 막는 것을 갈라 두어야 "정말 일어나는가"를
    /// 먼저 재 볼 수 있다.
    pub fn filter(&mut self, input: &[u8], protect: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            self.step(b, protect, &mut out);
        }
        out
    }

    fn step(&mut self, b: u8, protect: bool, out: &mut Vec<u8>) {
        // 붙들고 있는 것이 너무 길어지면 시퀀스가 아니다 — 흘려보내고 처음부터 본다.
        if self.pending.len() >= MAX_PENDING {
            out.append(&mut self.pending);
            self.reset();
        }
        match self.state {
            State::Ground => {
                if b == 0x1b {
                    self.pending.push(b);
                    self.state = State::Esc;
                } else {
                    out.push(b);
                }
            }
            State::Esc if b == b'[' => {
                self.pending.push(b);
                self.params.clear();
                self.cur = None;
                self.private = false;
                self.state = State::Params;
            }
            // `ESC` 다음이 `[` 가 아니면 우리 것이 아니다. 단, 또 `ESC` 면 새 시퀀스의 시작이다.
            State::Esc => {
                out.append(&mut self.pending);
                if b == 0x1b {
                    self.pending.push(b);
                } else {
                    out.push(b);
                    self.state = State::Ground;
                }
            }
            State::Params => self.param_byte(b, protect, out),
        }
    }

    fn param_byte(&mut self, b: u8, protect: bool, out: &mut Vec<u8>) {
        self.pending.push(b);
        match b {
            b'?' if self.params.is_empty() && self.cur.is_none() => self.private = true,
            b'0'..=b'9' => {
                let d = u32::from(b - b'0');
                self.cur = Some(self.cur.unwrap_or(0).saturating_mul(10).saturating_add(d).min(9999));
            }
            b';' => self.params.push(self.cur.take().unwrap_or(0)),
            b'J' => {
                if let Some(v) = self.cur.take() {
                    self.params.push(v);
                }
                // 사설 시퀀스(`CSI ? … J`)는 ED 가 아니다. 3 이 없으면 화면만 지우는 것이다.
                let is_wipe = !self.private && self.params.contains(&3);
                if is_wipe {
                    self.wipes = self.wipes.saturating_add(1);
                }
                if !(is_wipe && protect) {
                    out.append(&mut self.pending);
                }
                self.reset();
            }
            // 그 밖의 마침 바이트이거나 우리가 모르는 것 — 그대로 흘려보낸다.
            _ => {
                // ESC 는 **다음** 시퀀스의 것이다. 위에서 pending 에 넣어 두었으니 도로 빼서
                // 앞 조각과 함께 나가지 않게 한다(안 그러면 ESC 가 두 번 나간다).
                let starts_next = b == 0x1b;
                if starts_next {
                    self.pending.pop();
                }
                out.append(&mut self.pending);
                self.reset();
                if starts_next {
                    self.pending.push(0x1b);
                    self.state = State::Esc;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.state = State::Ground;
        self.pending.clear();
        self.params.clear();
        self.cur = None;
        self.private = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut WipeGuard, s: &[u8], protect: bool) -> Vec<u8> {
        g.filter(s, protect)
    }

    #[test]
    fn a_plain_stream_passes_through_untouched() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"hello\r\nworld", true), b"hello\r\nworld");
        assert_eq!(g.wipes(), 0);
    }

    #[test]
    fn the_wipe_is_removed_but_the_rest_survives() {
        let mut g = WipeGuard::default();
        let out = run(&mut g, b"a\x1b[3Jb", true);
        assert_eq!(out, b"ab", "지우기만 빠지고 나머지는 그대로여야 한다");
        assert_eq!(g.wipes(), 1);
    }

    /// 세기만 하고 막지 않는 모드 — "정말 일어나는가"를 먼저 재 볼 수 있어야 한다.
    #[test]
    fn counting_without_blocking_leaves_the_stream_alone() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"a\x1b[3Jb", false), b"a\x1b[3Jb");
        assert_eq!(g.wipes(), 1);
    }

    /// 화면만 지우는 것(`2J`)은 건드리지 않는다 — 그건 정상 동작이다.
    #[test]
    fn clearing_only_the_screen_is_left_alone() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"\x1b[2J", true), b"\x1b[2J");
        assert_eq!(g.wipes(), 0);
        assert_eq!(run(&mut g, b"\x1b[J", true), b"\x1b[J");
        assert_eq!(g.wipes(), 0);
    }

    /// PTY 는 아무 데서나 자른다 — 청크에 걸쳐 있어도 찾아내야 한다.
    #[test]
    fn it_survives_a_chunk_split_in_the_middle() {
        let mut g = WipeGuard::default();
        let mut out = run(&mut g, b"x\x1b[", true);
        out.extend(run(&mut g, b"3J", true));
        out.extend(run(&mut g, b"y", true));
        assert_eq!(out, b"xy");
        assert_eq!(g.wipes(), 1);
    }

    /// 사설 시퀀스는 ED 가 아니다(`CSI ? 3 J` 는 다른 뜻이다).
    #[test]
    fn private_sequences_are_not_ours() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"\x1b[?3J", true), b"\x1b[?3J");
        assert_eq!(g.wipes(), 0);
    }

    /// 여러 매개변수 중에 3 이 섞여 있어도 잡는다.
    #[test]
    fn it_finds_three_among_several_params() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"\x1b[1;3J", true), b"");
        assert_eq!(g.wipes(), 1);
    }

    /// 시퀀스 도중에 ESC 가 오면 그것이 새 시퀀스의 시작이다 — 다음 것을 놓치면 안 된다.
    #[test]
    fn an_esc_inside_a_sequence_starts_a_new_one() {
        let mut g = WipeGuard::default();
        let out = run(&mut g, b"\x1b[1\x1b[3J", true);
        assert_eq!(out, b"\x1b[1", "앞의 미완성 조각은 그대로 나가야 한다");
        assert_eq!(g.wipes(), 1);
    }

    /// 끝나지 않는 조각을 영원히 붙들지 않는다 — 화면이 멈춘 것처럼 보이면 안 된다.
    #[test]
    fn a_sequence_that_never_ends_is_eventually_let_go() {
        let mut g = WipeGuard::default();
        let long = [b"\x1b[".as_slice(), &[b'1'; MAX_PENDING + 8]].concat();
        let out = run(&mut g, &long, true);
        assert!(!out.is_empty(), "길어지면 흘려보내야 한다");
    }

    /// ESC 다음이 `[` 가 아닌 흔한 시퀀스(`ESC c` = 전체 리셋)를 삼키지 않는다.
    #[test]
    fn other_escape_sequences_pass_through() {
        let mut g = WipeGuard::default();
        assert_eq!(run(&mut g, b"\x1bc", true), b"\x1bc");
        assert_eq!(run(&mut g, b"\x1b]0;title\x07", true), b"\x1b]0;title\x07");
    }
}
