//! RGBA 색 + ANSI 16색 기본 팔레트.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    #[allow(clippy::self_named_constructors)]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// "#RRGGBB"/"RRGGBB" 또는 단축 "#RGB"/"RGB"를 파싱한다(형식 오류면 None).
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        match h.len() {
            6 => {
                let r = u8::from_str_radix(&h[0..2], 16).ok()?;
                let g = u8::from_str_radix(&h[2..4], 16).ok()?;
                let b = u8::from_str_radix(&h[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            // 단축형 #RGB → 각 자리 2배(f → ff).
            3 => {
                let n = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
                Some(Self::rgb(n(0)?, n(1)?, n(2)?))
            }
            _ => None,
        }
    }

    pub const BLACK: Self = Self::rgb(0x1e, 0x1e, 0x1e);
    pub const WHITE: Self = Self::rgb(0xdd, 0xdd, 0xdd);
}

impl Default for Rgba {
    fn default() -> Self {
        Self::WHITE
    }
}

/// ANSI 16색 인덱스(0..16)를 기본 팔레트 색으로 변환한다.
pub fn ansi16(index: u8) -> Rgba {
    const PALETTE: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x31, 0x31),
        (0x0d, 0xbc, 0x79),
        (0xe5, 0xe5, 0x10),
        (0x24, 0x72, 0xc8),
        (0xbc, 0x3f, 0xbc),
        (0x11, 0xa8, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x66, 0x66, 0x66),
        (0xf1, 0x4c, 0x4c),
        (0x23, 0xd1, 0x8b),
        (0xf5, 0xf5, 0x43),
        (0x3b, 0x8e, 0xea),
        (0xd6, 0x70, 0xd6),
        (0x29, 0xb8, 0xdb),
        (0xff, 0xff, 0xff),
    ];
    let (r, g, b) = PALETTE[(index & 0x0f) as usize];
    Rgba::rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_6_and_3_digit() {
        assert_eq!(Rgba::from_hex("#ff8800"), Some(Rgba::rgb(0xff, 0x88, 0x00)));
        assert_eq!(Rgba::from_hex("f80"), Some(Rgba::rgb(0xff, 0x88, 0x00)));
        assert_eq!(Rgba::from_hex("xyz"), None);
        assert_eq!(Rgba::from_hex(""), None);
    }
}
