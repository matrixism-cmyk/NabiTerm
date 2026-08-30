//! 폴더를 재귀로 훑을 때 **링크를 따라가지 않게** 하는 공통 판단.
//!
//! ## 왜 필요한가
//!
//! `Path::is_dir()` 는 링크를 따라간다. 그래서 폴더 안에 자기 부모를 가리키는 링크가
//! 하나 있으면(`a -> ..`) 재귀가 **끝없이 돈다.** 사용자는 멈춘 것으로 보고, 그동안
//! 디스크나 메모리는 계속 찬다.
//!
//! 폴더를 재귀로 도는 자리를 세어 보니 열다섯이었다(2026-08-30). 자리마다 따로 고치면
//! 다음에 새로 생기는 자리가 또 빠진다. 그래서 **묻는 방법을 하나로** 둔다.
//!
//! ## 왜 링크를 따라가지 않는 것이 맞는가
//!
//! 세 가지가 한꺼번에 해결된다.
//!
//! * **끝없이 도는 것을 막는다** — 고리가 있어도 한 번만 본다.
//! * **고르지 않은 것을 건드리지 않는다** — 폴더 밖을 가리키는 링크를 따라가면 사용자가
//!   고르지 않은 파일이 복사되거나 서버로 올라간다.
//! * **같은 것을 두 번 세지 않는다** — 크기·개수를 셀 때 링크 대상이 또 세어진다.
//!
//! 링크 자체를 보여 주는 것은 별개다. 목록에는 링크로 나오되, **들어가지 않는다.**

use std::path::Path;

/// 이 항목으로 **들어가도 되는 폴더**인가 — 링크면 아니다.
///
/// `is_dir()` 대신 이것을 쓴다. 한 번의 시스템 호출로 종류를 보고, 링크면 곧바로 거짓이다.
pub fn is_real_dir(path: &Path) -> bool {
    // `symlink_metadata` 는 링크를 따라가지 않는다 — 링크 자신의 정보를 준다.
    path.symlink_metadata().is_ok_and(|m| m.file_type().is_dir())
}

/// 이 항목이 링크인가.
pub fn is_link(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink())
}

#[cfg(test)]
mod tests {
    use super::{is_link, is_real_dir};

    /// 링크 하나를 실제로 만들어 본다. 윈도우는 권한이 필요해 못 만들면 건너뛴다 —
    /// 권한 없는 PC 에서 실패하면 아무도 이 시험을 안 보게 된다.
    #[test]
    fn 폴더를_가리키는_링크는_폴더로_보지_않는다() {
        let base = std::env::temp_dir().join(format!("nabi-walk-{}", std::process::id()));
        let real = base.join("real");
        if std::fs::create_dir_all(&real).is_err() {
            return;
        }
        let link = base.join("link");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&real, &link).is_ok();
        #[cfg(not(windows))]
        let made = std::os::unix::fs::symlink(&real, &link).is_ok();
        if !made {
            let _ = std::fs::remove_dir_all(&base);
            return;
        }
        assert!(is_real_dir(&real), "진짜 폴더는 폴더다");
        assert!(!is_real_dir(&link), "링크는 폴더로 보지 않는다");
        assert!(is_link(&link) && !is_link(&real));
        // 이것이 요지다 — 표준 `is_dir()` 은 링크를 따라가 참을 준다.
        assert!(link.is_dir(), "is_dir 는 링크를 따라간다(그래서 쓰면 안 된다)");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn 없는_경로는_폴더가_아니다() {
        let p = std::env::temp_dir().join("nabi-walk-nowhere-xyz");
        assert!(!is_real_dir(&p) && !is_link(&p));
    }
}
