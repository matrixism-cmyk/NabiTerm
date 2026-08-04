//! nabi-vt — per-pane 터미널 상태 모델.
//!
//! ByteChannel(ConPTY/SSH)이 흘려보내는 VT 바이트를 화면 상태로 파싱한다.
//! I/O·스레드 없음(바이트 in → 상태 out)이라 단위 테스트가 쉽다.
//!
//! 코어는 alacritty_terminal(T1 교체 완료). 렌더러는 `TermModel` 경계만 본다.

mod cache;
pub mod cell;
pub mod cursor;
mod dump;
pub mod grid;
mod images;
mod links;
mod prompts;
mod render;
mod search;

pub use cell::{CursorShape, RenderCell, Theme};
pub use cursor::CursorState;
pub use grid::TermModel;
