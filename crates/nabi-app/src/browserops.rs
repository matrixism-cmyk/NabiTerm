//! 로컬 파일시스템 조작/집계 헬퍼(복사·재귀 크기) — browserfs에서 분리(라인 한도).

use std::path::Path;

/// src를 dst_dir 아래로 복사한다(파일은 복사, 디렉터리는 재귀 복사).
///
/// **옮기지 못한 것이 몇 개인지 돌려준다.** 예전에는 결과를 버렸다. 폴더를 복사할 때
/// 그중 몇 개가 잠겨 있거나 권한이 없으면 그것만 빠지는데, 화면에는 아무 말도 없어서
/// 사용자는 전부 옮겨진 줄 안다. 나중에 원본을 지우면 그때 없어진다.
///
/// 실패했다고 멈추지는 않는다 — 하나가 안 된다고 나머지를 포기할 이유는 없다.
/// 옮길 수 있는 것은 옮기고, 못 옮긴 개수를 부른 쪽에 알린다.
pub(crate) fn copy_into(src: &Path, dst_dir: &Path) -> usize {
    let Some(name) = src.file_name() else { return 1 };
    let dst = dst_dir.join(name);
    if !src.is_dir() {
        return usize::from(std::fs::copy(src, &dst).is_err());
    }
    if std::fs::create_dir_all(&dst).is_err() {
        return 1; // 폴더를 못 만들면 그 안의 것도 다 못 옮긴다 — 하나로 센다.
    }
    let Ok(rd) = std::fs::read_dir(src) else { return 1 };
    rd.flatten().map(|e| copy_into(&e.path(), &dst)).sum()
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

/// 확장자 앞에 사본 표시를 넣은 이름. 선행 점(`.bashrc`)은 확장자가 아니다.
///
/// `word`는 화면 언어를 따른다. 예전에는 `" - copy"`가 영어로 박혀 있어서, 문구를
/// 1,790개나 번역해 둔 프로그램에서 **파일 이름만 영어로** 나왔다.
pub(crate) fn copy_suffixed(name: &str, n: usize, word: &str) -> String {
    let suffix = if n <= 1 {
        format!(" - {word}")
    } else {
        format!(" - {word} ({n})")
    };
    match name.rfind('.').filter(|&i| i > 0) {
        Some(i) => format!("{}{suffix}{}", &name[..i], &name[i..]),
        None => format!("{name}{suffix}"),
    }
}

/// dir 안의 name을 같은 폴더에 복제한다(충돌 시 번호 증가).
///
/// (새 이름, 못 옮긴 개수)를 돌려준다. 폴더를 복제할 때 그 안의 몇 개가 잠겨 있으면
/// 그것만 빠지는데, 개수를 안 돌려주면 부른 쪽이 알 길이 없다.
pub(crate) fn duplicate_in_dir(dir: &Path, name: &str, word: &str) -> Option<(String, usize)> {
    let src = dir.join(name);
    if !src.exists() {
        return None;
    }
    let new = (1..1000)
        .map(|n| copy_suffixed(name, n, word))
        .find(|c| !dir.join(c).exists())?;
    let dst = dir.join(&new);
    let mut failed = 0usize;
    if src.is_dir() {
        if std::fs::create_dir_all(&dst).is_err() {
            return None; // 대상 폴더를 못 만들면 복제 자체가 안 된 것이다.
        }
        let Ok(rd) = std::fs::read_dir(&src) else { return None };
        failed = rd.flatten().map(|e| copy_into(&e.path(), &dst)).sum();
    } else {
        std::fs::copy(&src, &dst).ok()?;
    }
    Some((new, failed))
}

#[cfg(test)]
mod tests {
    #[test]
    fn copy_suffix_inserts_before_extension() {
        use super::copy_suffixed;
        assert_eq!(copy_suffixed("file.txt", 1, "copy"), "file - copy.txt");
        assert_eq!(copy_suffixed("file.txt", 2, "copy"), "file - copy (2).txt");
        assert_eq!(copy_suffixed("Makefile", 1, "copy"), "Makefile - copy");
        assert_eq!(copy_suffixed(".bashrc", 1, "copy"), ".bashrc - copy"); // 선행 점은 확장자 아님.
        assert_eq!(copy_suffixed("a.tar.gz", 1, "copy"), "a.tar - copy.gz"); // 마지막 확장자 기준.
    }

    /// **낱말이 화면 언어를 따라야 한다.** 예전에는 영어로 박혀 있어서, 한국어 화면에서
    /// 파일 이름만 " - copy"로 나왔다.
    #[test]
    fn the_copy_word_follows_the_interface_language() {
        use super::copy_suffixed;
        assert_eq!(copy_suffixed("a.txt", 1, "사본"), "a - 사본.txt");
        assert_eq!(copy_suffixed("a.txt", 3, "コピー"), "a - コピー (3).txt");
    }

    #[test]
    fn duplicate_makes_unique_copy() {
        use super::duplicate_in_dir;
        let dir = std::env::temp_dir().join(format!("nabi-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), b"hi").unwrap();
        let (n1, f1) = duplicate_in_dir(&dir, "a.txt", "copy").unwrap();
        assert_eq!(f1, 0, "멀쩡한 파일은 하나도 빠지지 않는다");
        assert_eq!(n1, "a - copy.txt");
        assert_eq!(std::fs::read(dir.join(&n1)).unwrap(), b"hi");
        let (n2, _) = duplicate_in_dir(&dir, "a.txt", "copy").unwrap(); // 충돌 → 번호 증가.
        assert_eq!(n2, "a - copy (2).txt");
        assert!(duplicate_in_dir(&dir, "missing.txt", "copy").is_none());
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

    /// **못 옮긴 것을 실제로 세는가.** 세는 코드는 성공만 해 보면 늘 0 을 돌려주므로,
    /// 일부러 실패시켜 1 이 나오는지 확인해야 뜻이 있다.
    #[test]
    fn copy_into_counts_what_it_could_not_copy() {
        let base = std::env::temp_dir().join(format!("nabi-copyfail-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        // 없는 파일은 옮길 수 없다 — 하나로 세야 한다.
        assert_eq!(super::copy_into(&base.join("nowhere.txt"), &base), 1);
        let _ = std::fs::remove_dir_all(&base);
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
        assert_eq!(super::copy_into(&src, &dst), 0, "멀쩡한 것은 하나도 안 빠진다");
        assert_eq!(std::fs::read(dst.join("src").join("a.txt")).unwrap(), b"x");
        assert_eq!(std::fs::read(dst.join("src").join("sub").join("b.txt")).unwrap(), b"y");
        let _ = std::fs::remove_dir_all(&base);
    }
}
