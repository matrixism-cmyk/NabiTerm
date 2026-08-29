//! nabi-config — 계층 설정(figment) + 저장 위치 해석.
//!
//! 경로 해석(base 디렉터리)은 figment 레이어링보다 먼저 결정한다(닭-달걀 방지).
//! 사용자는 `NABI_CONFIG_DIR` 또는 포터블 모드로 저장 위치를 지정할 수 있다.

pub mod aiprofile;
pub mod editor;
pub mod load;
mod tolerant;
#[cfg(test)]
mod roundtrip_test;
pub mod paths;
pub mod persist;
pub mod schema;
pub mod telegram;
pub mod audit;

pub use editor::EditorConfig;
pub use load::{load, load_editor, load_reporting};
pub use paths::{resolve_base, StorageLayout};
pub use persist::save;
pub use aiprofile::AiProfileCfg;
pub use schema::{AppConfig, Appearance, TerminalCfg, DEFAULT_FONT_SIZE};
pub use telegram::TelegramCfg;
