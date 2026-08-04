//! 저장 위치 해석(단일 진실원).

use directories::ProjectDirs;
use std::path::PathBuf;

/// 모든 저장 파일 경로.
#[derive(Debug, Clone)]
pub struct StorageLayout {
    pub base: PathBuf,
    pub config_file: PathBuf,
    /// nabiPad(에디터) 전용 설정 — 터미널과 분리(독립 프로그램화 대비).
    pub editor_file: PathBuf,
    pub sessions_file: PathBuf,
    pub known_hosts: PathBuf,
    pub vault: PathBuf,
}

impl StorageLayout {
    /// base 디렉터리로부터 모든 경로를 구성한다.
    pub fn from_base(base: PathBuf) -> Self {
        Self {
            config_file: base.join("config.toml"),
            editor_file: base.join("nabipad.toml"),
            sessions_file: base.join("sessions.toml"),
            known_hosts: base.join("known_hosts"),
            vault: base.join("vault.bin"),
            base,
        }
    }

    /// 우선순위 체인으로 기본 레이아웃을 만든다.
    pub fn resolve() -> Self {
        Self::from_base(resolve_base())
    }
}

/// base 디렉터리 우선순위: `NABI_CONFIG_DIR` > 포터블(exe 옆) > ProjectDirs 기본.
pub fn resolve_base() -> PathBuf {
    if let Ok(dir) = std::env::var("NABI_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(p) = portable_base() {
        return p;
    }
    ProjectDirs::from("com", "aeo", "nabi")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 실행파일 옆에 `portable.toml` 마커가 있으면 그 디렉터리를 base로 쓴다(USB 설치).
fn portable_base() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    dir.join("portable.toml").exists().then_some(dir)
}
