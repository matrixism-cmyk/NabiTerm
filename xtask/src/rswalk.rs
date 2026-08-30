//! 검사기들이 쓰는 **러스트 파일 훑기** 한 벌.
//!
//! 세 검사기가 같은 훑기를 각자 적고 있었다. 셋 다 `is_dir()` 로 들어갔는데
//! **`is_dir()` 은 링크를 따라간다** — 자기 부모를 가리키는 링크가 하나 있으면 끝없이
//! 돈다. `crates` 아래를 도는 것은 `nabi_fs::walk::is_real_dir` 로 고쳤지만
//! (2026-08-30), xtask 는 그 크레이트를 안 쓰기 때문에 여기 남아 있었다.
//!
//! xtask 는 의존성이 없다(표준 라이브러리만 쓴다). 그래서 옮겨 오지 않고 여기 적는다.

use std::path::{Path, PathBuf};

/// 링크를 따라가지 않는 "진짜 폴더인가".
pub fn is_real_dir(p: &Path) -> bool {
    p.symlink_metadata().is_ok_and(|m| m.file_type().is_dir())
}

/// 이 아래의 모든 `.rs` 파일을 (경로, 내용) 으로 모은다. 읽지 못한 것은 건너뛴다.
pub fn rust_files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p: PathBuf = e.path();
            if is_real_dir(&p) {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    out.push((p.display().to_string(), s));
                }
            }
        }
    }
    out
}

/// 시험 코드를 뺀 본문. `#[cfg(test)]` 뒤는 보지 않는다.
pub fn without_tests(s: &str) -> &str {
    match s.find("#[cfg(test)]") {
        Some(i) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_real_dir, without_tests};

    #[test]
    fn 시험_앞까지만_본다() {
        assert_eq!(without_tests("a\n#[cfg(test)]\nb"), "a\n");
        assert_eq!(without_tests("a\nb"), "a\nb");
    }

    #[test]
    fn 없는_경로는_폴더가_아니다() {
        assert!(!is_real_dir(std::path::Path::new("이런 폴더는 없다")));
    }
}
