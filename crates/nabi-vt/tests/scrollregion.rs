//! 스크롤 영역(DECSTBM)으로 뷰포트 위에 히스토리를 밀어 넣는 TUI 방식이
//! 스크롤백에 쌓이는지 확인한다(codex CLI가 쓰는 insert_history 패턴).

use nabi_types::GridSize;
use nabi_vt::TermModel;

/// 영역 위쪽이 1행이면 밀려난 줄은 스크롤백으로 가야 한다.
#[test]
fn region_from_top_feeds_scrollback() {
    let mut m = TermModel::new(GridSize::new(20, 10), 100);
    m.process(b"\x1b[1;6r"); // 스크롤 영역 = 1~6행(그 아래는 TUI 뷰포트)
    m.process(b"\x1b[6;1H"); // 영역 맨 아랫줄로
    for i in 0..8 {
        m.process(format!("line{i}\r\n").as_bytes());
    }
    m.process(b"\x1b[r");
    println!("history_size = {}", m.history_size());
    assert!(m.history_size() > 0, "영역 상단이 1행이면 스크롤백에 쌓여야 한다");
}

/// SU(CSI S)로 영역을 밀어 올린 경우도 마찬가지.
#[test]
fn su_in_region_feeds_scrollback() {
    let mut m = TermModel::new(GridSize::new(20, 10), 100);
    m.process(b"\x1b[1;6r");
    m.process(b"\x1b[1;1Hhello");
    m.process(b"\x1b[3S"); // 영역 안에서 3줄 스크롤 업
    println!("history_size(SU) = {}", m.history_size());
    assert!(m.history_size() > 0, "SU도 스크롤백에 쌓여야 한다");
}
