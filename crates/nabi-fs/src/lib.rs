//! nabi-fs — 백엔드 무관 원격 파일시스템 추상화.
//!
//! 파일 브라우저 UI는 `RemoteFs` 트레잇만 사용한다. SFTP(`nabi-sftp`)·FTP(`nabi-ftp`)·
//! 로컬(`LocalFs`)이 같은 트레잇을 구현해 듀얼-페인 브라우저가 백엔드를 공유한다.

pub mod local;
pub mod remote_fs;

pub use local::LocalFs;
pub use remote_fs::{FileEntry, FileKind, RemoteFs};
