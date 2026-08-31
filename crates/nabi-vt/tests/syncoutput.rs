//! **동기화 출력**(DEC 사설 모드 2026)이 살아 있는가.
//!
//! ## 무엇인가
//!
//! 화면을 덮어 그리는 프로그램은 한 프레임을 여러 번에 나눠 보낸다. 그 중간이 화면에
//! 그려지면 반쯤 그린 상태가 보인다 — 사용자에게는 "글자가 깨졌다"로 보인다.
//!
//! 그래서 앱이 `ESC[?2026h` 로 "지금부터 한 덩어리다"라고 말하고, `ESC[?2026l` 로 끝을
//! 알린다. 그 사이의 것은 **다 모인 뒤에 한 번에** 화면에 반영된다.
//! 2026년 기준 마흔넷 중 열넷이 지원한다(조사 2026-09-01).
//!
//! ## 왜 시험이 필요한가
//!
//! 이 기능은 우리가 만든 것이 아니라 파서(vte)가 해 준다. 그래서 **우리가 모르는 채로
//! 깨뜨릴 수 있다** — 실제로 2026-08-31에 `wipeguard` 를 파서 **앞에** 끼워 넣었다.
//! 바이트를 먼저 걸러 내는 자리라, 거기서 시퀀스를 잘못 다루면 동기화가 조용히 죽는다.
//! 죽어도 화면은 나오므로 아무도 모른다 — 깜박임만 돌아온다.

use nabi_types::GridSize;
use nabi_vt::grid::TermModel;

/// 화면(스크롤백 말고)에 그 글자가 보이는가.
fn on_screen(m: &TermModel, needle: &str) -> bool {
    let top = m.top_abs_line();
    let rows = m.size().rows() as usize;
    m.lines_abs_text(top, top + rows).join("\n").contains(needle)
}

#[test]
fn what_is_inside_a_sync_block_does_not_show_until_it_ends() {
    let mut m = TermModel::new(GridSize::new(40, 5), 100);
    m.process(b"\x1b[?2026h"); // 여기서부터 한 덩어리.
    m.process(b"HALFWAY");
    assert!(!on_screen(&m, "HALFWAY"), "덩어리가 끝나기 전에 그려졌다 — 동기화가 죽었다");
    m.process(b"\x1b[?2026l"); // 끝.
    assert!(on_screen(&m, "HALFWAY"), "덩어리가 끝났는데도 안 나온다");
}

/// 동기화를 쓰지 않는 보통 출력은 곧바로 보여야 한다 — 막아 버리면 더 나쁘다.
#[test]
fn ordinary_output_still_shows_at_once() {
    let mut m = TermModel::new(GridSize::new(40, 5), 100);
    m.process(b"PLAIN");
    assert!(on_screen(&m, "PLAIN"));
}

/// 스크롤백 보호 필터가 **앞에 끼어 있어도** 동기화 시퀀스는 그대로 지나가야 한다.
///
/// 필터는 `ESC [` 를 보면 판단이 끝날 때까지 바이트를 들고 있는다. `?2026h` 는 사설
/// 시퀀스라 우리 것이 아니므로 그대로 흘려보내야 하는데, 그 길이 막히면 동기화가 죽는다.
#[test]
fn the_scrollback_guard_does_not_eat_the_sync_sequence() {
    let mut m = TermModel::new(GridSize::new(40, 5), 100);
    // 한 청크에 몰아서.
    m.process(b"\x1b[?2026hINSIDE\x1b[?2026l");
    assert!(on_screen(&m, "INSIDE"), "한 청크로 보낸 동기화 덩어리가 사라졌다");
}

/// PTY 는 아무 데서나 자른다 — 시퀀스가 청크 두 개에 걸쳐도 동기화가 살아야 한다.
#[test]
fn it_survives_a_chunk_split_inside_the_sequence() {
    let mut m = TermModel::new(GridSize::new(40, 5), 100);
    m.process(b"\x1b[?20");
    m.process(b"26h");
    m.process(b"SPLIT");
    assert!(!on_screen(&m, "SPLIT"), "쪼개진 시작 시퀀스를 못 알아봤다");
    m.process(b"\x1b[?2026l");
    assert!(on_screen(&m, "SPLIT"));
}
