//! 마우스 리포팅: 포인터 이벤트 → 터미널 마우스 보고 시퀀스(SGR/X10).

/// 보고할 마우스 버튼/휠.
#[derive(Clone, Copy)]
pub enum MouseBtn {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

/// 마우스 이벤트 1건을 보고 바이트로 인코딩한다. col/row는 0-based(내부).
/// `mods`는 xterm 수정자 비트(shift=4, meta=8, ctrl=16)로 버튼 코드에 OR된다.
pub fn mouse_report(sgr: bool, btn: MouseBtn, col: u16, row: u16, pressed: bool, mods: u16) -> Vec<u8> {
    let base: u16 = match btn {
        MouseBtn::Left => 0,
        MouseBtn::Middle => 1,
        MouseBtn::Right => 2,
        MouseBtn::WheelUp => 64,
        MouseBtn::WheelDown => 65,
        MouseBtn::WheelLeft => 66,
        MouseBtn::WheelRight => 67,
    };
    let code = base | mods;
    let c = col + 1;
    let r = row + 1;
    if sgr {
        let end = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{code};{c};{r}{end}").into_bytes()
    } else {
        // X10: ESC[M + (32+btn)(32+col)(32+row), 릴리스는 btn=3.
        let bcode = if pressed { code } else { 3 | mods };
        let clamp = |v: u16| (v + 32).min(255) as u8;
        vec![0x1b, b'[', b'M', clamp(bcode), clamp(c), clamp(r)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgr_press_and_release() {
        assert_eq!(mouse_report(true, MouseBtn::Left, 0, 0, true, 0), b"\x1b[<0;1;1M");
        assert_eq!(mouse_report(true, MouseBtn::Right, 4, 2, false, 0), b"\x1b[<2;5;3m");
    }

    #[test]
    fn sgr_with_ctrl_modifier() {
        // Ctrl(16) + Left(0) = 16.
        assert_eq!(mouse_report(true, MouseBtn::Left, 0, 0, true, 16), b"\x1b[<16;1;1M");
    }

    #[test]
    fn sgr_horizontal_wheel() {
        // WheelRight = 67.
        assert_eq!(
            mouse_report(true, MouseBtn::WheelRight, 0, 0, true, 0),
            b"\x1b[<67;1;1M"
        );
    }

    #[test]
    fn x10_press_left() {
        assert_eq!(
            mouse_report(false, MouseBtn::Left, 0, 0, true, 0),
            vec![0x1b, b'[', b'M', 32, 33, 33]
        );
    }
}
