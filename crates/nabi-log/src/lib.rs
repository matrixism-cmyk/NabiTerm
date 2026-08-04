//! nabi-log — tracing 설정 + 인-앱 로그 싱크 + span 헬퍼.
//!
//! 로깅은 워크스페이스 횡단 인프라라 한 크레이트로 격리해 nabi-app 밖에서도
//! (테스트 등) 재사용한다.

pub mod crash;
pub mod span;
pub mod subscriber;

pub use crash::install_crash_handler;
pub use span::pane_span;
pub use subscriber::init;
