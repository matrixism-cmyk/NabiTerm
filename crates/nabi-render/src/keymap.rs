//! 키 → 바이트 매핑 보조(입력 변환에서 사용).

use egui::{Key, Modifiers};

/// 키 조합을 사람이 읽는 단축키 라벨로(예: `Ctrl+Shift+C`). 단축키 도움말/팔레트 표시용.
pub fn key_label(key: Key, m: Modifiers) -> String {
    let mut s = String::new();
    if m.ctrl || m.command {
        s.push_str("Ctrl+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift {
        s.push_str("Shift+");
    }
    s.push_str(&format!("{key:?}")); // egui Key 변종 이름(예: "Enter", "ArrowUp", "C").
    s
}

/// 특수키/수정자 조합을 PTY 바이트 시퀀스로 변환한다(일반 문자는 Event::Text가 처리).
pub(crate) fn key_to_bytes(key: Key, m: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    if m.ctrl {
        if let Some(n) = ctrl_letter(key) {
            return Some(vec![n]);
        }
        // A~Z 외 표준 제어문자: Ctrl+Space=NUL, Ctrl+\ /] // = FS/GS/US.
        let extra = match key {
            Key::Space => Some(0u8),
            Key::Backslash => Some(0x1c),
            Key::CloseBracket => Some(0x1d),
            Key::Slash => Some(0x1f),
            _ => None,
        };
        if let Some(n) = extra {
            return Some(vec![n]);
        }
    }
    // 커서 키: 수정자(ctrl/alt/shift)가 있으면 xterm "CSI 1 ; mod letter",
    // 없으면 앱 모드 ESC O _, 일반 ESC [ _.
    let modcode = 1 + (m.shift as u8) + 2 * (m.alt as u8) + 4 * (m.ctrl as u8);
    let cur = |c: u8| -> Vec<u8> {
        if modcode > 1 {
            format!("\x1b[1;{modcode}{}", c as char).into_bytes()
        } else if app_cursor {
            vec![0x1b, b'O', c]
        } else {
            vec![0x1b, b'[', c]
        }
    };
    match key {
        Key::Enter => Some(b"\r".to_vec()),
        // Alt+Backspace = 단어 삭제(meta-DEL, readline backward-kill-word).
        Key::Backspace => Some(if m.alt { vec![0x1b, 0x7f] } else { vec![0x7f] }),
        // Shift+Tab = backtab(역방향 탭, 대화상자/TUI).
        Key::Tab => Some(if m.shift { b"\x1b[Z".to_vec() } else { b"\t".to_vec() }),
        Key::Escape => Some(b"\x1b".to_vec()),
        Key::ArrowUp => Some(cur(b'A')),
        Key::ArrowDown => Some(cur(b'B')),
        Key::ArrowRight => Some(cur(b'C')),
        Key::ArrowLeft => Some(cur(b'D')),
        Key::Home => Some(cur(b'H')),
        Key::End => Some(cur(b'F')),
        Key::Insert => Some(b"\x1b[2~".to_vec()),
        Key::Delete => Some(b"\x1b[3~".to_vec()),
        Key::PageUp => Some(b"\x1b[5~".to_vec()),
        Key::PageDown => Some(b"\x1b[6~".to_vec()),
        // 기능키(vim/htop/mc 등 TUI). F1~F4는 SS3, F5~F12는 CSI ~.
        Key::F1 => Some(b"\x1bOP".to_vec()),
        Key::F2 => Some(b"\x1bOQ".to_vec()),
        Key::F3 => Some(b"\x1bOR".to_vec()),
        Key::F4 => Some(b"\x1bOS".to_vec()),
        Key::F5 => Some(b"\x1b[15~".to_vec()),
        Key::F6 => Some(b"\x1b[17~".to_vec()),
        Key::F7 => Some(b"\x1b[18~".to_vec()),
        Key::F8 => Some(b"\x1b[19~".to_vec()),
        Key::F9 => Some(b"\x1b[20~".to_vec()),
        Key::F10 => Some(b"\x1b[21~".to_vec()),
        Key::F11 => Some(b"\x1b[23~".to_vec()),
        Key::F12 => Some(b"\x1b[24~".to_vec()),
        _ => None,
    }
}

/// Ctrl+letter → 제어 바이트(1..26). 그 외는 None.
pub(crate) fn ctrl_letter(key: Key) -> Option<u8> {
    let n = match key {
        Key::A => 1,
        Key::B => 2,
        Key::C => 3,
        Key::D => 4,
        Key::E => 5,
        Key::F => 6,
        Key::G => 7,
        Key::H => 8,
        Key::I => 9,
        Key::J => 10,
        Key::K => 11,
        Key::L => 12,
        Key::M => 13,
        Key::N => 14,
        Key::O => 15,
        Key::P => 16,
        Key::Q => 17,
        Key::R => 18,
        Key::S => 19,
        Key::T => 20,
        Key::U => 21,
        Key::V => 22,
        Key::W => 23,
        Key::X => 24,
        Key::Y => 25,
        Key::Z => 26,
        _ => return None,
    };
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::key_label;
    use egui::{Key, Modifiers};

    #[test]
    fn labels_modifiers() {
        assert_eq!(key_label(Key::C, Modifiers { ctrl: true, shift: true, ..Default::default() }), "Ctrl+Shift+C");
        assert_eq!(key_label(Key::Enter, Modifiers::default()), "Enter");
        assert_eq!(key_label(Key::ArrowUp, Modifiers { alt: true, ..Default::default() }), "Alt+ArrowUp");
    }
}
