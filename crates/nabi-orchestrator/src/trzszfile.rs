//! trzsz 전송의 디스크 쪽 — **원격이 준 이름으로 로컬에 쓰는** 자리라 여기가 제일 위험하다.
//!
//! 원격이 적대적이라고 가정한다. `../../.ssh/authorized_keys` 하나면 끝이다. 그래서 이름은
//! 통째로 거부하는 쪽을 택했다 — 고쳐서 살려 쓰지 않는다(고치다 보면 빠져나갈 길이 생긴다).

use nabi_trzsz::{FileSink, FileSource, Storage};
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

/// 저장 폴더 하나에 파일을 만드는 저장소.
pub struct DiskStorage {
    dir: PathBuf,
    /// 이번 전송에서 만든 파일 수 — 원격이 끝없이 보내는 것을 막는다.
    made: usize,
    max_files: usize,
}

impl DiskStorage {
    pub fn new(dir: PathBuf, max_files: usize) -> Self {
        Self { dir, made: 0, max_files }
    }
}

impl Storage for DiskStorage {
    fn create(&mut self, remote_name: &str, _size: u64) -> Result<(String, Box<dyn FileSink>), String> {
        if self.made >= self.max_files {
            return Err(format!("too many files in one transfer (limit {})", self.max_files));
        }
        let name = safe_name(remote_name)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot use save folder: {e}"))?;
        let path = unique_path(&self.dir, &name);
        let file = File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        self.made += 1;
        let local = path.file_name().map_or(name, |n| n.to_string_lossy().into_owned());
        Ok((local, Box::new(DiskSink { file: Some(file), path })))
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

    #[test]
    fn a_failed_transfer_leaves_no_file_behind() {
        let dir = std::env::temp_dir().join(format!("nabi-trzsz-f{}", std::process::id()));
        let mut st = DiskStorage::new(dir.clone(), 10);
        let (name, mut sink) = st.create("half.bin", 0).unwrap();
        sink.write(b"partial").unwrap();
        sink.finish(false).unwrap();
        assert!(!dir.join(&name).exists(), "취소·오류면 반쪽 파일은 지운다");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_count_is_capped() {
        let dir = std::env::temp_dir().join(format!("nabi-trzsz-c{}", std::process::id()));
        let mut st = DiskStorage::new(dir.clone(), 2);
        assert!(st.create("a", 0).is_ok());
        assert!(st.create("b", 0).is_ok());
        assert!(st.create("c", 0).is_err(), "원격이 끝없이 만들게 두면 안 된다");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
