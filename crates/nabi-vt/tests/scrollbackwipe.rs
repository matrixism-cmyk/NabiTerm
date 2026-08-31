//! **앱이 지나간 내용을 지우지 못하게 한다** — 사람이 시킨 것은 그대로 지운다.
//!
//! 2026-08-31 사용자 보고("휠을 올리면 예전 내용이 사라진다")를 재면서 만들었다.
//! 화면을 덮어 그리는 TUI 가 새로 그리기 전에 `CSI 3 J`(스크롤백 지우기)를 보내면,
//! 사람이 올려 보려던 것이 그 순간 없어진다. 지운 것은 되돌릴 수 없다.

use nabi_types::GridSize;
use nabi_vt::grid::TermModel;

/// 줄을 흘려 스크롤백을 쌓는다.
fn fill(m: &mut TermModel, n: usize) {
    for i in 0..n {
        m.process(format!("line {i}\r\n").as_bytes());
    }
}

#[test]
fn an_app_cannot_erase_what_the_person_scrolled_past() {
    let mut m = TermModel::new(GridSize::new(40, 5), 500);
    fill(&mut m, 60);
    let before = m.history_size();
    assert!(before > 0, "먼저 스크롤백이 쌓여 있어야 시험이 뜻을 갖는다");

    // 앱이 "스크롤백을 지워라"를 보낸다 — TUI 가 다시 그리기 전에 흔히 보낸다.
    m.process(b"\x1b[3J");
    assert_eq!(m.history_size(), before, "앱이 지나간 내용을 지웠다");
    assert_eq!(m.scrollback_wipes(), 1, "지우려 한 것은 세어 두어야 한다");
}

#[test]
fn the_person_can_still_clear_it_themselves() {
    let mut m = TermModel::new(GridSize::new(40, 5), 500);
    fill(&mut m, 60);
    assert!(m.history_size() > 0);
    // 메뉴의 "스크롤백 비우기" — 사람이 시킨 것은 막지 않는다.
    m.clear_scrollback();
    assert_eq!(m.history_size(), 0, "사람이 시켰는데 안 지워졌다");
}

#[test]
fn turning_the_guard_off_restores_the_old_behaviour() {
    let mut m = TermModel::new(GridSize::new(40, 5), 500);
    m.set_protect_scrollback(false);
    fill(&mut m, 60);
    assert!(m.history_size() > 0);
    m.process(b"\x1b[3J");
    assert_eq!(m.history_size(), 0, "끄면 예전처럼 앱이 지울 수 있어야 한다");
    assert_eq!(m.scrollback_wipes(), 1, "막지 않아도 세기는 한다");
}

/// 화면만 지우는 것(`2J`)은 원래 동작이다 — 건드리면 `clear` 가 이상해진다.
#[test]
fn clearing_the_screen_still_works() {
    let mut m = TermModel::new(GridSize::new(40, 5), 500);
    m.process(b"hello");
    m.process(b"\x1b[2J");
    // **화면만** 본다. `dump_text` 는 스크롤백까지 훑으므로 여기서는 답을 못 준다.
    let top = m.top_abs_line();
    let screen = m.lines_abs_text(top, top + 5).join("\n");
    assert!(!screen.contains("hello"), "화면 지우기가 막혔다: {screen:?}");
    assert_eq!(m.scrollback_wipes(), 0);
}
