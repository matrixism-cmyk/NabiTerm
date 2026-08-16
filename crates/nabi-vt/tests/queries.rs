//! 터미널 질의에 우리가 실제로 답하는지 확인한다.
//!
//! 무응답은 가장 나쁜 결과다 — 질의한 TUI는 응답을 기다리다 멈추거나, 색을 제멋대로
//! 짐작해 어두운 배경 위에 밝은 테마를 그린다.

use nabi_types::GridSize;
use nabi_vt::TermModel;

fn reply_to(seq: &[u8], theme: nabi_vt::Theme) -> String {
    let mut m = TermModel::new(GridSize::new(20, 5), 50);
    m.set_query_colors(&theme);
    m.process(seq);
    String::from_utf8_lossy(&m.take_replies()).into_owned()
}

/// OSC 11(배경색) 질의에 현재 테마 색으로 답한다.
#[test]
fn answers_background_color_query() {
    let t = nabi_vt::Theme { bg: nabi_types::Rgba::rgb(0x1e, 0x22, 0x2a), ..Default::default() };
    let r = reply_to(b"\x1b]11;?\x1b\\", t);
    assert!(r.contains("11;"), "OSC 11 응답이어야 한다: {r:?}");
    assert!(r.contains("1e") && r.contains("22") && r.contains("2a"), "테마 배경색: {r:?}");
}

/// OSC 10(전경색)도 마찬가지.
#[test]
fn answers_foreground_color_query() {
    let t = nabi_vt::Theme { fg: nabi_types::Rgba::rgb(0xd0, 0xd4, 0xdc), ..Default::default() };
    let r = reply_to(b"\x1b]10;?\x1b\\", t);
    assert!(r.contains("10;"), "OSC 10 응답이어야 한다: {r:?}");
    assert!(r.contains("d0") && r.contains("d4") && r.contains("dc"), "테마 전경색: {r:?}");
}

/// 장치 속성(DA1)은 예전부터 답하고 있었다 — 회귀 방지로 함께 고정한다.
#[test]
fn answers_device_attributes() {
    assert!(!reply_to(b"\x1b[c", nabi_vt::Theme::default()).is_empty());
}

/// Kitty keyboard protocol(T2-3): 협상은 코어가 하고 우리는 플래그를 읽는다.
/// push(`CSI > 1 u`) → 활성, pop(`CSI < u`) → 해제, 질의(`CSI ? u`)에는 응답이 있어야 한다.
#[test]
fn kitty_keyboard_negotiation_roundtrip() {
    let mut m = TermModel::new(GridSize::new(20, 5), 50);
    assert_eq!(m.kitty_keys(), 0, "초기엔 비활성");
    m.process(b"\x1b[>1u"); // disambiguate 푸시.
    assert_eq!(m.kitty_keys() & 1, 1, "disambiguate 활성");
    m.process(b"\x1b[?u"); // 질의 — 응답이 PTY로 가야 앱이 진행한다.
    assert!(!m.take_replies().is_empty(), "kitty 모드 질의에 응답해야 한다");
    m.process(b"\x1b[<u"); // 팝.
    assert_eq!(m.kitty_keys(), 0, "pop 후 비활성");
}
