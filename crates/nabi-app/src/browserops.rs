//! 로컬 파일시스템 조작/집계 헬퍼(복사·재귀 크기) — browserfs에서 분리(라인 한도).

use std::path::Path;

/// src를 dst_dir 아래로 복사한다(파일은 복사, 디렉터리는 재귀 복사).
pub(crate) fn copy_into(src: &Path, dst_dir: &Path) {
    let Some(name) = src.file_name() else { return };
    let dst = dst_dir.join(name);
    if src.is_dir() {
        let _ = std::fs::create_dir_all(&dst);
        if let Ok(rd) = std::fs::read_dir(src) {
            for e in rd.flatten() {
                copy_into(&e.path(), &dst);
            }
        }
    } else {
        let _ = std::fs::copy(src, &dst);
    }
}

/// 디렉터리 안 파일 개수와 총 바이트(재귀). 읽기 오류 항목은 건너뜀.
pub(crate) fn dir_stats(path: &Path) -> (u64, u64) {
    let (mut files, mut bytes) = (0u64, 0u64);
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let (f, b) = dir_stats(&p);
                files += f;
                bytes += b;
            } else if let Ok(md) = e.metadata() {
                files += 1;
                bytes += md.len();
            }
        }
    }
    (files, bytes)
}

/// 확장자 앞에 " - copy"(n>1이면 " - copy (n)")를 넣은 이름. 선행 점(.bashrc)은 확장자 아님.
pub(crate) fn copy_suffixed(name: &str, n: usize) -> String {
    let suffix = if n <= 1 {
        " - copy".to_string()
    } else {
        format!(" - copy ({n})")
    };
    match name.rfind('.').filter(|&i| i > 0) {
        Some(i) => format!("{}{suffix}{}", &name[..i], &name[i..]),
        None => format!("{name}{suffix}"),
    }
}

/// dir 안의 name을 같은 폴더에 복제한다(충돌 시 번호 증가). 성공 시 새 이름을 돌려준다.
pub(crate) fn duplicate_in_dir(dir: &Path, name: &str) -> Option<String> {
    let src = dir.join(name);
    if !src.exists() {
        return None;
    }
    let new = (1..1000)
        .map(|n| copy_suffixed(name, n))
        .find(|c| !dir.join(c).exists())?;
    let dst = dir.join(&new);
    if src.is_dir() {
        let _ = std::fs::create_dir_all(&dst);
        if let Ok(rd) = std::fs::read_dir(&src) {
            for e in rd.flatten() {
                copy_into(&e.path(), &dst);
            }
        }
    } else {
        std::fs::copy(&src, &dst).ok()?;
    }
    Some(new)
}

#[cfg(test)]
mod tests {
    #[test]
    fn copy_suffix_inserts_before_extension() {
        use super::copy_suffixed;
        assert_eq!(copy_suffixed("file.txt", 1), "file - copy.txt");
        assert_eq!(copy_suffixed("file.txt", 2), "file - copy (2).txt");
        assert_eq!(copy_suffixed("Makefile", 1), "Makefile - copy");
        assert_eq!(copy_suffixed(".bashrc", 1), ".bashrc - copy"); // 선행 점은 확장자 아님.
        assert_eq!(copy_suffixed("a.tar.gz", 1), "a.tar - copy.gz"); // 마지막 확장자 기준.
    }

    #[test]
    fn duplicate_makes_unique_copy() {
        use super::duplicate_in_dir;
        let dir = std::env::temp_dir().join(format!("nabi-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"hi").unwrap();
        let n1 = duplicate_in_dir(&dir, "a.txt").unwrap();
        assert_eq!(n1, "a - copy.txt");
        assert_eq!(std::fs::read(dir.join(&n1)).unwrap(), b"hi");
        let n2 = duplicate_in_dir(&dir, "a.txt").unwrap(); // 충돌 → 번호 증가.
        assert_eq!(n2, "a - copy (2).txt");
        assert!(duplicate_in_dir(&dir, "missing.txt").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dir_stats_recurses() {
        use super::dir_stats;
        let dir = std::env::temp_dir().join(format!("nabi-stats-{}", std::process::id()));
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.bin"), vec![0u8; 100]).unwrap();
        std::fs::write(sub.join("b.bin"), vec![0u8; 50]).unwrap();
        assert_eq!(dir_stats(&dir), (2, 150)); // 2개 파일, 150바이트(하위 포함).
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_into_recurses_dir() {
        let base = std::env::temp_dir().join(format!("nabi-copytest-{}", std::process::id()));
        let src = base.join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"x").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"y").unwrap();
        let dst = base.join("dst");
        std::fs::create_dir_all(&dst).unwrap();
        super::copy_into(&src, &dst);
        assert_eq!(std::fs::read(dst.join("src").join("a.txt")).unwrap(), b"x");
        assert_eq!(std::fs::read(dst.join("src").join("sub").join("b.txt")).unwrap(), b"y");
        let _ = std::fs::remove_dir_all(&base);
    }
}
