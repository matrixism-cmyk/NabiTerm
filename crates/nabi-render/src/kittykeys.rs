//! Kitty keyboard protocol 인코더(T2-3) — "점진적 키보드 향상"의 disambiguate 계층.
//!
//! 협상(CSI = flags u push/pop)은 코어(alacritty_terminal)가 이미 처리한다. 여기서는
//! 활성 플래그일 때 키를 스펙대로 인코딩만 한다. AI CLI(claude 등)가 Shift+Enter
//! 멀티라인·Ctrl+I/Tab 구분에 이 프로토콜을 쓴다(Windows Terminal 1.25도 채택).
//!
//! v1 범위(disambiguate=1 비트): ① Esc → `CSI 27u` ② 수정자 붙은 Enter/Tab/Backspace →
//! `CSI 13;mod u` 형태 ③ Ctrl/Alt 붙은 인쇄 키 → `CSI 코드포인트;mod u`
//! ④ 그 외는 기존(legacy) 인코딩 유지 — 스펙도 disambiguate 계층에선 그렇게 요구한다.

use egui::{Key, Modifiers};

/// kitty 플래그 비트(스펙 그대로).
pub const DISAMBIGUATE: u8 = 1;

/// kitty 수정자 값: 1 + shift(1)+alt(2)+ctrl(4)+super(8).
fn modcode(m: Modifiers) -> u8 {
    1 + (m.shift as u8) + 2 * (m.alt as u8) + 4 * ((m.ctrl || m.command) as u8)
}

/// `CSI code;mod u` (mod 1이면 생략).
fn csi_u(code: u32, m: Modifiers) -> Vec<u8> {
    let mc = modcode(m);
    if mc > 1 {
        format!("\x1b[{code};{mc}u").into_bytes()
    } else {
        format!("\x1b[{code}u").into_bytes()
    }
}

/// egui Key → 기본 계층(소문자) 코드포인트. 인쇄 가능한 키만.
fn printable_code(key: Key) -> Option<u32> {
    let name = key.symbol_or_name();
    let mut it = name.chars();
    match (it.next(), it.next()) {
        // 한 글자짜리 키 이름(A~Z, 0~9, 구두점) — 소문자 기본 계층으로.
        (Some(c), None) => Some(c.to_ascii_lowercase() as u32),
        _ => (key == Key::Space).then_some(' ' as u32),
    }
}

/// disambiguate 활성 시 키 인코딩. None이면 호출자가 legacy 경로로 폴백한다.
///
/// 반환 Some(vec![])은 "이 키는 여기서 삼킨다"가 아니라 쓰지 않는다 — 항상 시퀀스가 있다.
pub(crate) fn key_to_bytes(key: Key, m: Modifiers, flags: u8) -> Option<Vec<u8>> {
    if flags & DISAMBIGUATE == 0 {
        return None;
    }
    let plain = !m.shift && !m.alt && !m.ctrl && !m.command;
    match key {
        // Esc는 항상 CSI 27u — 앱이 "Esc 단독"과 "ESC 시퀀스 시작"을 구분하게 하는 핵심.
        Key::Escape => Some(csi_u(27, m)),
        // Enter/Tab/Backspace: 수정자 없으면 legacy(CR/HT/DEL) 유지가 스펙. 있으면 CSI u —
        // 이 덕에 Shift+Enter가 Enter와 구분된다(AI CLI 멀티라인).
        Key::Enter if !plain => Some(csi_u(13, m)),
        Key::Tab if !plain => Some(csi_u(9, m)),
        Key::Backspace if !plain => Some(csi_u(127, m)),
        // Ctrl/Alt 붙은 인쇄 키: 제어문자로 뭉개지 않고 CSI 코드포인트;mod u.
        _ if (m.ctrl || m.command || m.alt) => printable_code(key).map(|c| csi_u(c, m)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };
    const CTRL: Modifiers = Modifiers { ctrl: true, ..Modifiers::NONE };

    #[test]
    fn inactive_flags_fall_back() {
        assert_eq!(key_to_bytes(Key::Escape, Modifiers::NONE, 0), None);
    }

    #[test]
    fn esc_is_csi_27u() {
        assert_eq!(key_to_bytes(Key::Escape, Modifiers::NONE, 1).unwrap(), b"\x1b[27u");
    }

    #[test]
    fn shift_enter_disambiguates() {
        // 멀티라인 입력의 핵심 — Enter(CR)와 Shift+Enter(CSI 13;2u)가 달라진다.
        assert_eq!(key_to_bytes(Key::Enter, SHIFT, 1).unwrap(), b"\x1b[13;2u");
        // 수정자 없는 Enter는 legacy 유지(폴백).
        assert_eq!(key_to_bytes(Key::Enter, Modifiers::NONE, 1), None);
    }

    #[test]
    fn ctrl_i_differs_from_tab() {
        // legacy에선 Ctrl+I == Tab(0x09) — disambiguate에선 CSI 105;5u로 구분된다.
        assert_eq!(key_to_bytes(Key::I, CTRL, 1).unwrap(), b"\x1b[105;5u");
        assert_eq!(key_to_bytes(Key::Tab, CTRL, 1).unwrap(), b"\x1b[9;5u");
    }

    #[test]
    fn plain_text_keys_stay_legacy() {
        // 수정자 없는 인쇄 키는 Event::Text가 처리 — 여기선 폴백.
        assert_eq!(key_to_bytes(Key::A, Modifiers::NONE, 1), None);
        assert_eq!(key_to_bytes(Key::ArrowUp, Modifiers::NONE, 1), None);
    }
}
