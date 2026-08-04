//! nabi-editor — 외부 편집기 편집.
//!
//! 원격 파일을 임시로 받아 외부 편집기로 열고, 임시 파일이 바뀌면 재업로드한다.
//! 백엔드는 `nabi_fs::RemoteFs`라 SFTP/FTP/로컬 어디서나 동작한다.
//!
//! 핵심 사이클(다운로드→재업로드)은 LocalFs로 단위 테스트한다. 편집기 실행/파일 감시는
//! 후속에 notify로 자동화한다(현재는 폴링형 `reupload_if_changed`).

pub mod editor;

pub use editor::EditSession;
