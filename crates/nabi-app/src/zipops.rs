//! **폴더에서 zip으로 묶고 푼다** — 올리기 전에 묶고, 받은 것을 풀 때 밖으로 나가지 않게.
//!
//! zip 라이브러리는 이미 있다(소프트웨어 GL 자산을 푸는 데 쓴다). 브라우저에서 쓸 길만
//! 없었다.
//!
//! ## 푸는 쪽이 위험하다
//!
//! zip 안의 경로는 **파일을 만든 쪽이 정한다.** `../../Windows/System32/...` 같은 이름이
//! 들어 있으면, 그대로 이어 붙이는 순간 푸는 폴더 **밖에** 파일을 쓰게 된다(zip slip).
//! 그래서 항목마다 경로를 검사해 밖을 가리키면 **건너뛰고 세어 둔다.**
//!
//! ## 상한
//!
//! 묶는 것도 푸는 것도 한도를 둔다. 압축 폭탄(작은 zip이 수십 GB로 풀리는 것)을 만나면
//! 디스크가 차기 전에 멈춰야 하고, 멈췄다는 사실을 **말해야** 한다.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// 한 번에 다룰 최대 항목 수.
pub(crate) const MAX_ENTRIES: usize = 20_000;
/// 풀어낼 최대 총 바이트(압축 폭탄 방어).
pub(crate) const MAX_TOTAL: u64 = 4 << 30;

/// 묶기·풀기 결과 요약. 조용히 빠뜨리지 않으려고 건너뛴 수를 함께 센다.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ZipReport {
    pub done: usize,
    /// 안전하지 않은 경로라 건너뛴 항목 수(zip slip).
    pub unsafe_paths: usize,
    /// 상한에 걸려 멈췄나.
    pub truncated: bool,
}

/// zip 안의 이름을 **푸는 폴더 안쪽** 상대 경로로 바꾼다. 밖을 가리키면 None.
///
/// 막는 것: 절대 경로(`/etc`, `C:\`), 위로 올라가기(`..`), 드라이브 접두(`C:`),
/// 그리고 윈도우가 구분자로 읽는 역빗금.
pub(crate) fn safe_rel(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    // zip은 `/`를 쓰지만 윈도우에서 만든 것에는 역빗금이 섞여 온다.
    let unified = name.replace('\\', "/");
    let p = Path::new(&unified);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => out.push(s),
            // 그 밖은 전부 밖을 가리킬 수 있는 조각이다 — 하나라도 있으면 통째로 거절한다.
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// 고른 항목들을 `dest` zip으로 묶는다(폴더는 하위까지).
pub(crate) fn create(root: &Path, names: &[String], dest: &Path) -> Result<ZipReport, String> {
    let f = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut w = zip::ZipWriter::new(f);
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut rep = ZipReport::default();
    for name in names {
        let path = root.join(name);
        if nabi_fs::walk::is_real_dir(&path) {
            add_dir(&mut w, &path, Path::new(name), &opts, &mut rep)?;
        } else {
            add_file(&mut w, &path, name, &opts, &mut rep)?;
        }
        if rep.truncated {
            break;
        }
    }
    w.finish().map_err(|e| e.to_string())?;
    Ok(rep)
}

fn add_file<W: Write + std::io::Seek>(
    w: &mut zip::ZipWriter<W>,
    path: &Path,
    name: &str,
    opts: &zip::write::FileOptions<'_, ()>,
    rep: &mut ZipReport,
) -> Result<(), String> {
    if rep.done >= MAX_ENTRIES {
        rep.truncated = true;
        return Ok(());
    }
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    w.start_file(name.replace('\\', "/"), *opts).map_err(|e| e.to_string())?;
    w.write_all(&data).map_err(|e| e.to_string())?;
    rep.done += 1;
    Ok(())
}

fn add_dir<W: Write + std::io::Seek>(
    w: &mut zip::ZipWriter<W>,
    dir: &Path,
    rel: &Path,
    opts: &zip::write::FileOptions<'_, ()>,
    rep: &mut ZipReport,
) -> Result<(), String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(()); // 못 읽는 폴더는 건너뛴다(권한 등) — 묶기 전체를 멈출 일은 아니다.
    };
    for e in rd.filter_map(|e| e.ok()) {
        if rep.truncated {
            return Ok(());
        }
        let child = e.path();
        let crel = rel.join(e.file_name());
        let cname = crel.to_string_lossy().into_owned();
        // 링크를 따라가면 끝없이 돈다 — 압축이 끝나지 않는다.
        match nabi_fs::walk::is_real_dir(&child) {
            true => add_dir(w, &child, &crel, opts, rep)?,
            false => add_file(w, &child, &cname, opts, rep)?,
        }
    }
    Ok(())
}

/// `src` zip을 `dest` 폴더에 푼다. 밖을 가리키는 항목은 **건너뛰고 센다**.
pub(crate) fn extract(src: &Path, dest: &Path) -> Result<ZipReport, String> {
    let f = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut rep = ZipReport::default();
    let mut total = 0u64;
    for i in 0..z.len() {
        if rep.done >= MAX_ENTRIES || total >= MAX_TOTAL {
            rep.truncated = true;
            break;
        }
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = safe_rel(e.name()) else {
            rep.unsafe_paths += 1;
            continue;
        };
        let out = dest.join(rel);
        if e.is_dir() {
            let _ = std::fs::create_dir_all(&out);
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|x| x.to_string())?;
        }
        let mut buf = Vec::new();
        e.read_to_end(&mut buf).map_err(|x| x.to_string())?;
        total += buf.len() as u64;
        std::fs::write(&out, &buf).map_err(|x| x.to_string())?;
        rep.done += 1;
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::{create, extract, safe_rel};
    use std::path::Path;

    #[test]
    fn a_plain_name_is_kept() {
        assert_eq!(safe_rel("a/b.txt").unwrap(), Path::new("a").join("b.txt"));
    }

    /// **이 시험이 푸는 쪽의 존재 이유다** — zip 안의 이름이 폴더 밖을 가리킬 수 있다.
    #[test]
    fn a_path_that_escapes_is_refused() {
        for evil in ["../x", "a/../../x", "/etc/passwd", "..\\..\\win.ini", "C:/Windows/x"] {
            assert!(safe_rel(evil).is_none(), "빠져나가는 경로를 받아들였다: {evil}");
        }
    }

    #[test]
    fn an_empty_or_dotted_name_is_refused() {
        assert!(safe_rel("").is_none());
        assert!(safe_rel(".").is_none());
        assert!(safe_rel("..").is_none());
    }

    /// 윈도우에서 만든 zip은 역빗금을 쓴다 — 그것도 폴더로 읽어야 한다.
    #[test]
    fn backslashes_are_treated_as_folders() {
        assert_eq!(safe_rel("a\\b.txt").unwrap(), Path::new("a").join("b.txt"));
    }

    /// 묶었다 풀면 **내용이 같아야 한다**(왕복).
    #[test]
    fn a_round_trip_keeps_the_contents() {
        let base = std::env::temp_dir().join("nabi_zip_rt");
        let (src, out) = (base.join("src"), base.join("out"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"hello").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"world").unwrap();
        let zipfile = base.join("out.zip");
        let rep = create(&src, &["a.txt".into(), "sub".into()], &zipfile).unwrap();
        assert_eq!(rep.done, 2, "{rep:?}");
        let rep2 = extract(&zipfile, &out).unwrap();
        assert_eq!(rep2.done, 2);
        assert_eq!(std::fs::read(out.join("a.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(out.join("sub").join("b.txt")).unwrap(), b"world");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// 한글 이름도 왕복해야 한다(우리 사용자의 파일 이름이 그렇다).
    #[test]
    fn hangul_names_survive_the_round_trip() {
        let base = std::env::temp_dir().join("nabi_zip_ko");
        let (src, out) = (base.join("src"), base.join("out"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("보고서.txt"), "안녕".as_bytes()).unwrap();
        let zipfile = base.join("k.zip");
        create(&src, &["보고서.txt".into()], &zipfile).unwrap();
        extract(&zipfile, &out).unwrap();
        assert_eq!(std::fs::read(out.join("보고서.txt")).unwrap(), "안녕".as_bytes());
        let _ = std::fs::remove_dir_all(&base);
    }
}
