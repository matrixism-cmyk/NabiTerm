//! 세션 트리 파일 저장/로드(원자적).

use crate::export::{from_toml, to_toml};
use crate::model::SessionTree;
use std::io::Write;
use std::path::Path;

/// sessions.toml에서 트리를 로드한다(없거나 깨지면 빈 트리).
pub fn load_tree(path: &Path) -> SessionTree {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| from_toml(&s).ok())
        .unwrap_or_default()
}

/// 트리를 원자적으로 저장한다(임시파일 → fsync → rename).
pub fn save_tree(path: &Path, tree: &SessionTree) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = to_toml(tree).map_err(to_io)?;
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
