//! 셀 텍스트 속성(굵게/기울임/밑줄/반전 등) 비트셋과 색 참조.

use serde::{Deserialize, Serialize};

/// 셀 표시 속성 비트셋.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CellAttrs(pub u16);

impl CellAttrs {
    pub const BOLD: u16 = 1 << 0;
    pub const ITALIC: u16 = 1 << 1;
    pub const UNDERLINE: u16 = 1 << 2;
    pub const INVERSE: u16 = 1 << 3;
    pub const STRIKE: u16 = 1 << 4;
    pub const DIM: u16 = 1 << 5;
    pub const HIDDEN: u16 = 1 << 6;
    /// East Asian 이중폭 글리프를 담은 셀(2칸 차지).
    pub const WIDE: u16 = 1 << 7;
    /// 이중폭 글리프 뒤를 채우는 셀(렌더 시 그리지 않음).
    pub const WIDE_SPACER: u16 = 1 << 8;
    // 밑줄 스타일(UNDERLINE=단일과 별개) — SGR 4:2/4:3/4:4/4:5.
    pub const UNDERCURL: u16 = 1 << 9;
    pub const DOUBLE_UNDERLINE: u16 = 1 << 10;
    pub const DOTTED_UNDERLINE: u16 = 1 << 11;
    pub const DASHED_UNDERLINE: u16 = 1 << 12;
    /// 모든 밑줄 계열(렌더에서 "밑줄 있음" 판정용).
    pub const ANY_UNDERLINE: u16 = Self::UNDERLINE
        | Self::UNDERCURL
        | Self::DOUBLE_UNDERLINE
        | Self::DOTTED_UNDERLINE
        | Self::DASHED_UNDERLINE;

    pub const fn contains(self, mask: u16) -> bool {
        self.0 & mask != 0
    }
    pub fn set(&mut self, mask: u16, on: bool) {
        if on {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }
}


