//! 스크롤백 검색 내비게이션(Find F3/Enter 백엔드) 런타임 검증.

use nabi_types::GridSize;
use nabi_vt::search::MatchScan;
use nabi_vt::TermModel;

#[test]
fn scroll_prev_match_finds_scrollback() {
    let mut m = TermModel::new(GridSize::new(80, 24), 1000);
    m.process(b"needle here\r\n");
    for i in 0..40 {
        m.process(format!("filler line {i}\r\n").as_bytes());
    }
    // 위로 스크롤하면 스크롤백의 needle 줄을 찾는다(술어 기반 — 리터럴 contains).
    assert_eq!(m.scroll_to_prev_match(|l| l.contains("needle"), 500), MatchScan::Found);
    // 없는 검색어는 맨 위까지 스캔 후 false.
    assert_eq!(m.scroll_to_prev_match(|l| l.contains("zzznotfound"), 500), MatchScan::NotFound);
}

// (T1 해결) vt100 0.15.2의 offset>화면행수 언더플로 한계가 alacritty 코어 교체로 사라져
// 한 화면보다 깊은 스크롤백 검색도 debug 빌드에서 정상 동작한다.
#[test]
fn scroll_prev_match_deep_scrollback() {
    let mut m = TermModel::new(GridSize::new(80, 24), 1000);
    m.process(b"deepneedle\r\n");
    for i in 0..80 {
        m.process(format!("line {i}\r\n").as_bytes());
    }
    assert_eq!(m.scroll_to_prev_match(|l| l.contains("deepneedle"), 500), MatchScan::Found);
}
