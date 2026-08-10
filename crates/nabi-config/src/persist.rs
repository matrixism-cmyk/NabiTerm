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
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    // Windows rename은 기존 대상 교체를 보장하지 않는다. 먼저 ReplaceFileW를 쓰고,
    // 대상이 아직 없을 때만 일반 rename으로 폴백한다.
    replace_file(&tmp, path)
}

#[cfg(windows)]
fn replace_file(tmp: &Path, path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
    }
    if !path.exists() {
        return std::fs::rename(tmp, path);
    }
    let dst: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let src: Vec<u16> = tmp.as_os_str().encode_wide().chain([0]).collect();
    let ok = unsafe {
        ReplaceFileW(
            dst.as_ptr(),
            src.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, path: &Path) -> std::io::Result<()> {
    std::fs::rename(tmp, path)
}

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
