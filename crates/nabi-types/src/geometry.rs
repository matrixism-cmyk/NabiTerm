//! 그리드/픽셀 기하 타입.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cols(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rows(pub u16);

/// 터미널 그리드 크기(열·행).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSize {
    pub cols: Cols,
    pub rows: Rows,
}

impl GridSize {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: Cols(cols),
            rows: Rows(rows),
        }
    }
    pub const fn cols(self) -> u16 {
        self.cols.0
    }
    pub const fn rows(self) -> u16 {
        self.rows.0
    }
    pub const fn area(self) -> usize {
        self.cols.0 as usize * self.rows.0 as usize
    }
}

impl Default for GridSize {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

