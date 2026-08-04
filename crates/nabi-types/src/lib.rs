//! nabi-types — 의존성 없는 공용 어휘(ID/기하/색/속성).
//!
//! 모든 크레이트가 의존하는 안정적 기반. 로직·I/O 없음.
//! 순환 의존을 막고 PaneId 등 식별자를 한 곳에서 정의한다.

pub mod attrs;
pub mod color;
pub mod geometry;
pub mod ids;

pub use attrs::CellAttrs;
pub use color::{ansi16, Rgba};
pub use geometry::{Cols, GridSize, Rows};
pub use ids::{next_pane_id, next_window_id, PaneId, WindowId};
