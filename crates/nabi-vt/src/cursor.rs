//! 커서 상태 타입(추출은 TermModel::cursor).

/// 커서 위치와 표시 여부.
#[derive(Clone, Copy, Debug)]
pub struct CursorState {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}
