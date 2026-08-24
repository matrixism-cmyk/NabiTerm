//! nabi-session — 저장 세션 트리 + 내보내기/가져오기.
//!
//! ⚠️ 비밀은 절대 직렬화하지 않는다(세션 파일엔 vault 참조만; export 시 단언으로 보장).
//! 포맷은 버전드(schema_version) — 후속 마이그레이션 가능.

pub mod export;
pub mod groups;
pub mod model;
#[cfg(test)]
mod model_tests;
pub mod store;

pub use export::{from_json, from_toml, to_json, to_toml, SessionExport};
pub use model::{session_matches, SavedSession, SessionKind, SessionTag, SessionTree};
pub use store::{load_tree, load_tree_reporting, save_tree};
