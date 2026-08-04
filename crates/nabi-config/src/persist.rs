//! 설정 원자적 저장.

use serde::Serialize;
use std::io::Write;
use std::path::Path;

/// 설정을 원자적으로 저장한다(임시파일 작성 → fsync → rename).
///
/// 싸구려 USB/동기화 미디어에서도 부분 쓰기로 깨지지 않게 한다.
/// `AppConfig`·`EditorConfig` 등 직렬화 가능한 어떤 설정에도 쓴다(단일 진실원).
pub fn save<T: Serialize>(path: &Path, cfg: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = toml::to_string_pretty(cfg).map_err(to_io)?;
    let tmp = path.with_extension("toml.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
