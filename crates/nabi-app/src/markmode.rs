//! **표시 모드** — 마우스 없이 터미널 글을 고르고 복사한다.
//!
//! ## 왜 필요한가
//!
//! 지금까지 터미널에서 글을 고르는 길은 **마우스뿐**이었다(끌기, 링크 길게 누르기,
//! "전체 선택"). 키보드만으로는 화면에 보이는 한 줄조차 복사할 수 없었다.
//!
//! 이것은 두 가지 문제다.
//!
//! * **손을 옮겨야 한다.** 터미널을 키보드로 쓰는 사람에게 복사만 마우스인 것은 어색하다.
//!   tmux 의 copy-mode, 윈도우 터미널의 Mark Mode 가 같은 자리를 메운다.
//! * **마우스를 못 쓰면 아예 길이 없다.** 접근성 문제이고, 이 저장소는 실제로 휠이
//!   말없이 버려지던 결함을 겪었다 — 마우스에만 기대는 길은 끊기면 대안이 없다.
//!
//! ## 어떻게 움직이나
//!
//! 여기 있는 것은 **순수 계산**이다. 화면 크기와 지금 자리를 받아 다음 자리를 돌려준다.
//! 실제 그리기·복사·키 가로채기는 부르는 쪽이 한다 — 그래야 실서버도 화면도 없이 시험할 수 있다.
//!
//! ## 왜 새 단축키를 안 만드나
//!
//! 이 저장소는 단축키를 늘리지 않기로 했다(이미 많다). 들어가는 길은 **명령 팔레트**이고,
//! 들어간 뒤에는 **모드 안에서만** 키가 뜻을 갖는다 — 모드가 켜져 있는 동안 그 키들은
//! 셸로 가지 않으므로 기존 키와 부딪히지 않는다.

/// 표시 모드에서 할 수 있는 움직임.
///
/// 열거형으로 두는 까닭: 키를 여기서 뜻으로 바꿔 두면, 키 배치를 바꿔도 움직임 규칙과
/// 그 시험은 그대로다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Move {
    Left,
    Right,
    Up,
    Down,
    /// 줄 맨 앞 / 맨 끝.
    Home,
    End,
    /// 한 화면 위 / 아래.
    PageUp,
    PageDown,
    /// 글 맨 위 / 맨 아래.
    Top,
    Bottom,
}

/// 화면 크기(칸 수). 0 이면 아무 데도 못 가므로 1 로 본다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Grid {
    pub rows: usize,
    pub cols: usize,
}

impl Grid {
    fn last_row(&self) -> usize {
        self.rows.saturating_sub(1)
    }
    fn last_col(&self) -> usize {
        self.cols.saturating_sub(1)
    }
}

/// 커서를 옮긴다. **화면 밖으로 나가지 않는다.**
///
/// 왼쪽 끝에서 더 왼쪽으로 가면 윗줄 끝으로 넘어간다(글을 읽는 차례와 같다).
/// 맨 위·맨 아래에서는 그냥 멈춘다 — 여기서 스크롤백까지 다루면 규칙이 두 배가 되고,
/// 지금 필요한 것은 "보이는 화면에서 고르기"다.
pub(crate) fn step(g: Grid, (r, c): (usize, usize), m: Move) -> (usize, usize) {
    let (lr, lc) = (g.last_row(), g.last_col());
    let (r, c) = (r.min(lr), c.min(lc));
    match m {
        Move::Left => match c {
            0 if r > 0 => (r - 1, lc),
            0 => (0, 0),
            _ => (r, c - 1),
        },
        Move::Right => match c >= lc {
            true if r < lr => (r + 1, 0),
            true => (lr.min(r), lc),
            false => (r, c + 1),
        },
        Move::Up => (r.saturating_sub(1), c),
        Move::Down => ((r + 1).min(lr), c),
        Move::Home => (r, 0),
        Move::End => (r, lc),
        Move::PageUp => (r.saturating_sub(g.rows), c),
        Move::PageDown => ((r + g.rows).min(lr), c),
        Move::Top => (0, 0),
        Move::Bottom => (lr, lc),
    }
}

/// 키를 움직임으로. 모드가 모르는 키는 `None` — 부르는 쪽이 그냥 흘린다.
///
/// **셸로 보내지 않는다.** 모드가 켜진 동안은 여기서 모르는 키도 삼킨다 — 안 그러면
/// 글을 고르는 도중에 명령이 실행된다.
pub(crate) fn key_to_move(key: egui::Key, m: &egui::Modifiers) -> Option<Move> {
    use egui::Key as K;
    Some(match key {
        K::ArrowLeft => Move::Left,
        K::ArrowRight => Move::Right,
        K::ArrowUp => Move::Up,
        K::ArrowDown => Move::Down,
        // Ctrl+Home/End 는 글 끝으로 — 편집기의 관행 그대로다.
        K::Home if m.ctrl => Move::Top,
        K::End if m.ctrl => Move::Bottom,
        K::Home => Move::Home,
        K::End => Move::End,
        K::PageUp => Move::PageUp,
        K::PageDown => Move::PageDown,
        _ => return None,
    })
}

use crate::selection::Sel;
use crate::tabs::TermTabViewer;
use nabi_types::{GridSize, PaneId};

impl TermTabViewer<'_> {
    /// 표시 모드의 키를 처리한다. **여기 오는 키는 셸로 가지 않는다.**
    ///
    /// * 화살표·Home/End·PageUp/Down — 커서를 옮긴다.
    /// * Shift 를 누른 채 — 고른 범위를 넓힌다(누르지 않으면 그 자리에서 다시 시작).
    /// * Enter / Ctrl+C — 고른 것을 복사하고 모드를 끝낸다.
    /// * Esc — 아무것도 안 하고 끝낸다.
    pub(crate) fn handle_mark_keys(&mut self, ui: &mut egui::Ui, pane: PaneId, grid: GridSize) {
        let g = Grid { rows: grid.rows() as usize, cols: grid.cols() as usize };
        // 모드에 들어온 직후에는 고른 것이 없다 — 화면 맨 위 왼쪽에서 시작한다.
        let cur = match self.selection.as_ref().filter(|s| s.pane == pane) {
            Some(s) => (s.hr, s.hc),
            None => (0, 0),
        };
        let (keys, m) = ui.input(|i| {
            let ks: Vec<egui::Key> = i.events.iter().filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, .. } => Some(*key),
                _ => None,
            }).collect();
            (ks, i.modifiers)
        });
        for k in keys {
            if k == egui::Key::Escape {
                *self.mark_mode = None;
                *self.selection = None;
                return;
            }
            // Enter 또는 Ctrl+C 로 확정 — 복사는 부르는 쪽(앱)이 한 길로 한다.
            if k == egui::Key::Enter || (k == egui::Key::C && m.ctrl) {
                *self.mark_mode = None;
                *self.mark_copy = true;
                return;
            }
            let Some(mv) = key_to_move(k, &m) else { continue };
            let (nr, nc) = step(g, cur, mv);
            let anchor = match (self.selection.as_ref().filter(|s| s.pane == pane), m.shift) {
                // Shift 를 누르고 있으면 시작점은 그대로 두고 끝만 옮긴다.
                (Some(s), true) => (s.ar, s.ac),
                // 아니면 그 자리에서 새로 시작한다.
                _ => (nr, nc),
            };
            *self.selection = Some(Sel { pane, ar: anchor.0, ac: anchor.1, hr: nr, hc: nc, rect: false });
            ui.ctx().request_repaint();
        }
    }
}

impl crate::app::NabiApp {
    /// 표시 모드를 켠다(명령 팔레트에서).
    ///
    /// 켤 때 고른 것을 비운다 — 마우스로 골라 둔 것이 남아 있으면 첫 화살표가 그것을
    /// 넓히는지 새로 시작하는지 알 수 없다.
    pub(crate) fn start_mark_mode(&mut self) {
        let Some(p) = self.focused_pane() else {
            self.notify = Some((nabi_i18n::tr(self.lang, "mark.nopane").to_string(), std::time::Instant::now()));
            return;
        };
        self.mark_mode = Some(p);
        self.selection = None;
        // 무엇을 누르면 되는지 그 자리에서 말해 준다 — 모드에 들어왔는데 아무 말이
        // 없으면 키보드가 먹통이 된 것처럼 보인다.
        self.notify = Some((nabi_i18n::tr(self.lang, "mark.on").to_string(), std::time::Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Grid = Grid { rows: 24, cols: 80 };

    /// **화면 밖으로 절대 안 나간다.** 나가면 그 자리로 글을 꺼내려다 패닉이 난다
    /// (이 저장소는 인덱스 초과로 렌더러가 죽어 앱이 즉시 종료된 적이 있다).
    #[test]
    fn 화면_밖으로_안_나간다() {
        let all = [
            Move::Left, Move::Right, Move::Up, Move::Down, Move::Home,
            Move::End, Move::PageUp, Move::PageDown, Move::Top, Move::Bottom,
        ];
        for start in [(0, 0), (23, 79), (0, 79), (23, 0), (12, 40)] {
            for m in all {
                let (r, c) = step(G, start, m);
                assert!(r < G.rows && c < G.cols, "{start:?} + {m:?} = ({r},{c})");
            }
        }
    }

    /// 왼쪽 끝에서 더 가면 **윗줄 끝**으로 — 글을 읽는 차례와 같다.
    #[test]
    fn 줄을_넘어_이어진다() {
        assert_eq!(step(G, (5, 0), Move::Left), (4, 79));
        assert_eq!(step(G, (5, 79), Move::Right), (6, 0));
    }

    /// 맨 앞·맨 끝에서는 멈춘다(넘어갈 데가 없다).
    #[test]
    fn 끝에서는_멈춘다() {
        assert_eq!(step(G, (0, 0), Move::Left), (0, 0));
        assert_eq!(step(G, (23, 79), Move::Right), (23, 79));
        assert_eq!(step(G, (0, 5), Move::Up), (0, 5));
        assert_eq!(step(G, (23, 5), Move::Down), (23, 5));
    }

    /// 한 화면씩 움직인다.
    #[test]
    fn 한_화면씩_움직인다() {
        assert_eq!(step(G, (23, 3), Move::PageUp), (0, 3), "24줄 화면이니 맨 위로");
        assert_eq!(step(G, (0, 3), Move::PageDown), (23, 3));
    }

    /// **크기가 0인 화면에서도 죽지 않는다.** 창이 아주 작아지면 실제로 이런 값이 온다.
    #[test]
    fn 빈_화면에서도_죽지_않는다() {
        let z = Grid { rows: 0, cols: 0 };
        for m in [Move::Left, Move::Right, Move::Up, Move::Down, Move::Bottom, Move::PageDown] {
            assert_eq!(step(z, (0, 0), m), (0, 0), "{m:?}");
        }
    }

    /// 지금 자리가 화면 밖이어도(창이 줄어든 직후) 안으로 끌어들인다.
    #[test]
    fn 화면_밖에서_시작해도_안으로_들어온다() {
        let (r, c) = step(G, (999, 999), Move::Up);
        assert!(r < G.rows && c < G.cols, "({r},{c})");
    }

    /// 키가 뜻으로 바뀐다 — 그리고 **모르는 키는 None** 이어야 부르는 쪽이 갈래를 나눈다.
    #[test]
    fn 키가_뜻으로_바뀐다() {
        let none = egui::Modifiers::NONE;
        assert_eq!(key_to_move(egui::Key::ArrowLeft, &none), Some(Move::Left));
        assert_eq!(key_to_move(egui::Key::Home, &none), Some(Move::Home));
        assert_eq!(key_to_move(egui::Key::A, &none), None);
        // Ctrl+Home 은 줄 맨 앞이 아니라 글 맨 위다.
        let ctrl = egui::Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_to_move(egui::Key::Home, &ctrl), Some(Move::Top));
        assert_eq!(key_to_move(egui::Key::End, &ctrl), Some(Move::Bottom));
    }
}
