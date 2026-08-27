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
    // SAFETY: dst·src는 NUL로 끝나는 UTF-16 벡터이고 호출 동안 살아 있다. 나머지 인자는
    // 규격상 선택이라 null을 넘긴다(백업 파일·예약 필드 없음).
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

#[cfg(test)]
mod tests {
    use super::save;

    // TOML 은 최상위가 표여야 한다 — 벡터를 그냥 넘기면 "unsupported rust type" 이 난다.
    // (처음에 그렇게 적었다가 시험이 잡았다.)
    fn doc(v: i64) -> std::collections::BTreeMap<String, i64> {
        std::collections::BTreeMap::from([("n".to_string(), v)])
    }

    /// **임시 파일 이름에 프로세스 번호가 들어가는가.**
    ///
    /// 안 들어가면 nabiTerm 을 두 개 띄웠을 때 같은 임시 파일을 놓고 다툰다 — 한쪽이 반쯤
    /// 쓴 파일을 다른 쪽이 제자리로 옮기면 저장된 것이 깨진다. 세션 저장이 실제로 그랬고,
    /// 이 함수로 모으면서 고쳤다. 다시 갈라지지 않게 여기서 지킨다.
    #[test]
    fn the_temp_name_is_unique_per_process() {
        let dir = std::env::temp_dir().join(format!("nabi-persist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.toml");
        save(&f, &doc(3)).unwrap();
        assert!(f.exists(), "저장된 파일이 있어야 한다");
        // 임시 파일은 남지 않는다(rename 으로 옮겨졌다).
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "임시 파일이 남았다: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_twice_replaces_rather_than_appends() {
        // 윈도우 rename 은 기존 대상 교체를 보장하지 않는다 — 실제로 교체되는지 본다.
        let dir = std::env::temp_dir().join(format!("nabi-persist2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("y.toml");
        save(&f, &doc(1)).unwrap();
        save(&f, &doc(22)).unwrap();
        let body = std::fs::read_to_string(&f).unwrap();
        assert!(body.contains("22"), "두 번째 저장이 반영돼야 한다: {body:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
