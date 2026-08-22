//! trzsz 전송의 디스크 쪽 — **원격이 준 이름으로 로컬에 쓰는** 자리라 여기가 제일 위험하다.
//!
//! 원격이 적대적이라고 가정한다. `../../.ssh/authorized_keys` 하나면 끝이다. 그래서 이름은
//! 통째로 거부하는 쪽을 택했다 — 고쳐서 살려 쓰지 않는다(고치다 보면 빠져나갈 길이 생긴다).

use nabi_trzsz::{Entry, FileSink, FileSource, Storage};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Windows에서 이름만으로 장치가 되어 버리는 예약어(확장자를 붙여도 마찬가지다).
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 원격이 보낸 이름이 **저장 폴더 안의 파일 하나**를 가리키는지 검사한다.
///
/// 통과하면 그 이름을 그대로 돌려준다. 하나라도 걸리면 거절한다 — 경로 구분자,
/// 상위 참조, 드라이브 문자, UNC, NTFS 대체 스트림, 예약어, 제어문자.
pub fn safe_name(name: &str) -> Result<String, String> {
    let bad = |why: &str| Err(format!("unsafe file name from remote ({why}): {name}"));
    if name.is_empty() || name.len() > 255 {
        return bad("length");
    }
    if name.contains('/') || name.contains('\\') {
        return bad("path separator");
    }
    if name == "." || name == ".." {
        return bad("relative path");
    }
    if name.contains(':') {
        return bad("drive or alternate data stream");
    }
    if name.chars().any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*')) {
        return bad("control or reserved character");
    }
    // Windows는 이름 끝의 공백·마침표를 조용히 잘라낸다 — 검사한 이름과 만들어지는 이름이 달라진다.
    if name.ends_with(' ') || name.ends_with('.') {
        return bad("trailing space or dot");
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return bad("reserved device name");
    }
    Ok(name.to_owned())
}

/// 겹치지 않는 경로를 고른다 — **덮어쓰지 않는다**. `a.txt` → `a (1).txt` → `a (2).txt`.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for n in 1..10_000 {
        let p = dir.join(format!("{stem} ({n}){ext}"));
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem} (dup){ext}"))
}

/// 경로 조각 수 상한 — 원격이 끝없이 깊은 트리를 만들지 못하게 한다.
const MAX_DEPTH: usize = 32;

/// 저장 폴더 하나에 파일·폴더를 만드는 저장소.
pub struct DiskStorage {
    dir: PathBuf,
    /// 이번 전송에서 만든 항목 수 — 원격이 끝없이 보내는 것을 막는다.
    made: usize,
    max_files: usize,
    /// 최상위 이름이 겹쳐 바뀌었을 때의 새 이름(path_id별). 같은 폴더의 나머지 항목이
    /// **같은 새 이름 아래**로 들어가야 폴더가 둘로 갈라지지 않는다.
    roots: HashMap<i64, String>,
}

impl DiskStorage {
    pub fn new(dir: PathBuf, max_files: usize) -> Self {
        Self { dir, made: 0, max_files, roots: HashMap::new() }
    }

    /// 이 항목이 실제로 놓일 경로. 조각을 하나씩 검사하고, 마지막에 **저장 폴더 안인지**
    /// 다시 확인한다(검사와 실제 경로가 어긋나는 틈을 남기지 않는다).
    fn resolve(&mut self, entry: &Entry) -> Result<(String, PathBuf), String> {
        if entry.rel.len() > MAX_DEPTH {
            return Err(format!("path too deep from remote ({} parts)", entry.rel.len()));
        }
        // 조각은 하나하나가 이름이어야 한다 — `..`도 `\`도 여기서 걸린다.
        let parts: Vec<String> =
            entry.rel.iter().map(|p| safe_name(p)).collect::<Result<_, _>>()?;
        let (root, rest) = parts.split_first().ok_or("empty path from remote")?;

        // 최상위 이름은 path_id마다 한 번만 정한다(폴더가 갈라지지 않게).
        let local = match self.roots.get(&entry.path_id) {
            Some(v) => v.clone(),
            None => {
                let chosen = unique_path(&self.dir, root.as_str())
                    .file_name()
                    .map_or_else(|| root.clone(), |n| n.to_string_lossy().into_owned());
                self.roots.insert(entry.path_id, chosen.clone());
                chosen
            }
        };

        let mut path = self.dir.join(&local);
        for p in rest {
            path.push(p);
        }
        // 최종 확인: 정규화한 결과가 저장 폴더 아래인가. 조각 검사를 통과했더라도
        // 심볼릭 링크·정션이 걸려 있으면 밖으로 나갈 수 있다.
        let base = self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone());
        let check = path.parent().and_then(|p| p.canonicalize().ok());
        if let Some(parent) = check {
            if !parent.starts_with(&base) {
                return Err(format!("remote path escapes the save folder: {}", path.display()));
            }
        }
        Ok((local, path))
    }
}

impl Storage for DiskStorage {
    fn create(&mut self, entry: &Entry) -> Result<(String, Option<Box<dyn FileSink>>), String> {
        if self.made >= self.max_files {
            return Err(format!("too many items in one transfer (limit {})", self.max_files));
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot use save folder: {e}"))?;
        let (local, path) = self.resolve(entry)?;
        self.made += 1;
        if entry.is_dir {
            std::fs::create_dir_all(&path)
                .map_err(|e| format!("cannot create folder {}: {e}", path.display()))?;
            return Ok((local, None));
        }
        // 중간 폴더는 조용히 만든다 — 원격이 파일부터 보내는 구현도 있다.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create folder {}: {e}", parent.display()))?;
        }
        // 폴더 안의 파일은 이미 고유한 자리다. 최상위 파일 하나뿐일 때만 겹침을 피한다.
        let path = if entry.rel.len() == 1 { unique_path(&self.dir, &local) } else { path };
        let file = File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        let local = if entry.rel.len() == 1 {
            path.file_name().map_or(local, |n| n.to_string_lossy().into_owned())
        } else {
            local
        };
        Ok((local, Some(Box::new(DiskSink { file: Some(file), path }))))
    }
}

/// 받은 바이트를 파일에 쓴다. 끝이 좋지 않으면 **지운다**(반쪽 파일을 남기지 않는다).
struct DiskSink {
    file: Option<File>,
    path: PathBuf,
}

impl FileSink for DiskSink {
    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        let f = self.file.as_mut().ok_or("file already closed")?;
        f.write_all(data).map_err(|e| format!("write failed: {e}"))
    }

    fn finish(&mut self, ok: bool) -> Result<(), String> {
        let file = self.file.take();
        if let Some(f) = file {
            let flushed = f.sync_all();
            drop(f);
            if ok {
                return flushed.map_err(|e| format!("flush failed: {e}"));
            }
        }
        if !ok {
            let _ = std::fs::remove_file(&self.path);
        }
        Ok(())
    }
}

/// 보낼 파일을 읽는다.
pub struct DiskSource(File);

impl DiskSource {
    /// 파일을 열고 (크기, 원본)을 돌려준다.
    pub fn open(path: &Path) -> Result<(u64, Self), String> {
        let f = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        let size = f.metadata().map_err(|e| format!("cannot stat: {e}"))?.len();
        Ok((size, Self(f)))
    }
}

impl FileSource for DiskSource {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        self.0.read(buf).map_err(|e| format!("read failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_names_including_hangul() {
        assert_eq!(safe_name("report.txt").unwrap(), "report.txt");
        assert_eq!(safe_name("보고서 최종.hwp").unwrap(), "보고서 최종.hwp");
        assert_eq!(safe_name(".bashrc").unwrap(), ".bashrc");
    }

    /// 여기 있는 것 하나라도 통과하면 원격이 로컬 파일 시스템을 건드릴 수 있다.
    #[test]
    fn refuses_every_escape_shape() {
        for bad in [
            "",
            "..",
            ".",
            "../etc/passwd",
            "..\\windows\\system32\\drivers\\etc\\hosts",
            "/etc/passwd",
            "C:\\Windows\\evil.dll",
            "sub/dir.txt",
            "notes.txt:hidden",       // NTFS 대체 스트림
            "\\\\server\\share\\x",   // UNC
            "CON",
            "com1.txt",
            "nul",
            "bad\u{0}name",
            "trailing ",
            "trailing.",
            "a<b>.txt",
        ] {
            assert!(safe_name(bad).is_err(), "이걸 통과시키면 안 된다: {bad:?}");
        }
    }

    #[test]
    fn long_names_are_refused() {
        assert!(safe_name(&"a".repeat(300)).is_err());
        assert!(safe_name(&"a".repeat(200)).is_ok());
    }

    #[test]
    fn unique_path_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("nabi-trzsz-t{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let first = unique_path(&dir, "a.txt");
        assert_eq!(first.file_name().unwrap(), "a.txt");
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(&dir, "a.txt");
        assert_eq!(second.file_name().unwrap(), "a (1).txt");
        std::fs::write(&second, b"x").unwrap();
        assert_eq!(unique_path(&dir, "a.txt").file_name().unwrap(), "a (2).txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_path_handles_names_without_extension() {
        let dir = std::env::temp_dir().join(format!("nabi-trzsz-n{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("README"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "README").file_name().unwrap(), "README (1)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tmp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nabi-trzsz-{tag}{}", std::process::id()))
    }

    #[test]
    fn a_failed_transfer_leaves_no_file_behind() {
        let dir = tmp("f");
        let mut st = DiskStorage::new(dir.clone(), 10);
        let (name, sink) = st.create(&Entry::file("half.bin", 7)).unwrap();
        let mut sink = sink.expect("파일이면 쓸 곳이 있어야 한다");
        sink.write(b"partial").unwrap();
        sink.finish(false).unwrap();
        assert!(!dir.join(&name).exists(), "취소·오류면 반쪽 파일은 지운다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_count_is_capped() {
        let dir = tmp("c");
        let mut st = DiskStorage::new(dir.clone(), 2);
        assert!(st.create(&Entry::file("a", 0)).is_ok());
        assert!(st.create(&Entry::file("b", 0)).is_ok());
        assert!(st.create(&Entry::file("c", 0)).is_err(), "원격이 끝없이 만들게 두면 안 된다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn entry(id: i64, parts: &[&str], is_dir: bool) -> Entry {
        Entry {
            path_id: id,
            rel: parts.iter().map(|s| (*s).to_owned()).collect(),
            is_dir,
            size: 0,
            perm: None,
        }
    }

    /// 폴더 전송: 디렉터리는 만들고 쓸 곳은 없다. 안쪽 파일은 그 아래에 놓인다.
    #[test]
    fn a_folder_lands_as_a_folder() {
        let dir = tmp("d");
        let mut st = DiskStorage::new(dir.clone(), 100);
        let (root, sink) = st.create(&entry(0, &["docs"], true)).unwrap();
        assert_eq!(root, "docs");
        assert!(sink.is_none(), "디렉터리에는 쓸 곳이 없다");
        assert!(dir.join("docs").is_dir());

        let (root2, sink) = st.create(&entry(0, &["docs", "img", "a.png"], false)).unwrap();
        assert_eq!(root2, "docs", "같은 전송의 최상위 이름은 하나여야 한다");
        sink.expect("파일").write(b"x").unwrap();
        assert!(dir.join("docs").join("img").join("a.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 최상위 이름이 겹쳐 바뀌면 그 폴더의 나머지도 **같은 새 이름 아래**로 가야 한다.
    #[test]
    fn a_renamed_folder_does_not_split_in_two() {
        let dir = tmp("r");
        let _ = std::fs::create_dir_all(dir.join("docs"));
        let mut st = DiskStorage::new(dir.clone(), 100);
        let (root, _) = st.create(&entry(5, &["docs"], true)).unwrap();
        assert_eq!(root, "docs (1)", "겹치면 새 이름을 고른다");
        let (root2, sink) = st.create(&entry(5, &["docs", "b.txt"], false)).unwrap();
        assert_eq!(root2, "docs (1)");
        sink.expect("파일").write(b"y").unwrap();
        assert!(dir.join("docs (1)").join("b.txt").exists());
        assert!(!dir.join("docs").join("b.txt").exists(), "원래 폴더를 건드리면 안 된다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 폴더 전송에서도 조각 하나하나가 검사된다 — 여기가 뚫리면 P4가 곧 취약점이다.
    #[test]
    fn every_path_part_is_checked_not_just_the_first() {
        let dir = tmp("e");
        let mut st = DiskStorage::new(dir.clone(), 100);
        for bad in [
            entry(0, &["docs", "..", "escaped.txt"], false),
            entry(0, &["docs", r"..\evil.txt"], false),
            entry(0, &["docs", "sub/evil.txt"], false),
            entry(0, &["docs", "CON"], false),
        ] {
            assert!(st.create(&bad).is_err(), "통과시키면 안 된다: {:?}", bad.rel);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_absurdly_deep_path_is_refused() {
        let dir = tmp("p");
        let mut st = DiskStorage::new(dir.clone(), 100);
        let deep: Vec<String> = (0..64).map(|i| format!("d{i}")).collect();
        let e = Entry { path_id: 0, rel: deep, is_dir: false, size: 0, perm: None };
        assert!(st.create(&e).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
