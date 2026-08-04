//! nabi-error — 공유 에러 스캐폴딩.
//!
//! 각 크레이트는 자기 thiserror enum을 소유한다. 이 크레이트는 여러 곳이
//! 공유하는 횡단 변형과 Result 규약, 컨텍스트 헬퍼만 호스팅한다.

pub mod context;
pub mod kind;

pub use context::Contextual;
pub use kind::{SharedError, SharedResult};
