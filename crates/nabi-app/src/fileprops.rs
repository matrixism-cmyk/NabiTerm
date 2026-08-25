//! 파일 **속성** — 크기·시각·특성·해시를 한자리에서.
//!
//! 탐색기·FileZilla 모두 기본으로 갖춘 화면인데 우리에겐 없었다(감사 2026-08-25).
//! "이 파일 언제 것이지", "받은 파일이 원본과 같나"를 확인하려면 다른 프로그램을 열어야 했다.
//!
//! ## 해시는 요청해야 계산한다
//!
//! SHA-256은 파일 전체를 읽어야 나온다. 속성 창을 열 때마다 자동으로 돌면 큰 파일에서
//! 창이 멈춘 것처럼 보인다. 그래서 단추를 눌렀을 때만, 그리고 **곁 스레드에서** 읽는다.

use std::path::{Path, PathBuf};

/// 한 파일·폴더의 속성 스냅샷(계산이 끝난 값만).
#[derive(Clone, Default)]
pub(crate) struct Props {
    pub path: PathBuf,
    pub is_dir: bool,
    pub bytes: u64,
    pub modified: Option<std::time::SystemTime>,
    pub created: Option<std::time::SystemTime>,
    pub readonly: bool,
    /// 폴더일 때만: (파일 수, 총 바이트). 계산 전에는 None.
    pub dir_total: Option<(u64, u64)>,
    /// SHA-256(요청 후 계산). 계산 중에는 `Some(None)`.
    pub sha256: Option<Option<String>>,
}

/// 경로를 훑어 즉시 알 수 있는 값만 채운다(파일 내용은 읽지 않는다).
pub(crate) fn read(path: &Path) -> Option<Props> {
    let md = std::fs::metadata(path).ok()?;
    Some(Props {
        path: path.to_path_buf(),
        is_dir: md.is_dir(),
        bytes: md.len(),
        modified: md.modified().ok(),
        created: md.created().ok(),
        readonly: md.permissions().readonly(),
        dir_total: None,
        sha256: None,
    })
}

/// 시각을 사람이 읽는 문자열로. 못 읽으면 빈 문자열.
pub(crate) fn stamp(t: Option<std::time::SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 파일 이름에서 확장자를 뽑는다(표시용, 없으면 빈 문자열).
pub(crate) fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// 파일을 흘려 읽어 SHA-256을 낸다 — **문서를 메모리에 모으지 않는다.**
///
/// 곁 스레드에서 부른다. 큰 파일에서 창이 멈춘 것처럼 보이지 않게.
pub(crate) fn sha256_of(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("nabi-props-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn it_reads_what_the_filesystem_already_knows() {
        let d = tmp("read");
        let f = d.join("hello.TXT");
        std::fs::write(&f, b"12345").unwrap();
        let p = read(&f).expect("파일이 있으면 읽어야 한다");
        assert_eq!(p.bytes, 5);
        assert!(!p.is_dir);
        assert!(p.modified.is_some());
        assert_eq!(ext_of(&p.path), "txt", "확장자는 소문자로");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 속성을 읽는 것만으로는 **내용을 읽지 않는다** — 해시는 요청해야 돈다.
    #[test]
    fn reading_properties_does_not_hash_the_file() {
        let d = tmp("nohash");
        let f = d.join("big.bin");
        std::fs::write(&f, vec![7u8; 4096]).unwrap();
        assert!(read(&f).unwrap().sha256.is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 해시는 알려진 값과 맞아야 한다(빈 파일의 SHA-256은 표준 상수다).
    #[test]
    fn the_hash_matches_the_known_value() {
        let d = tmp("hash");
        let f = d.join("empty");
        std::fs::write(&f, b"").unwrap();
        assert_eq!(
            sha256_of(&f).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let g = d.join("abc");
        std::fs::write(&g, b"abc").unwrap();
        assert_eq!(
            sha256_of(&g).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 1MB 버퍼 경계를 넘는 파일도 한 덩어리로 읽은 것과 같아야 한다.
    #[test]
    fn a_file_larger_than_the_read_buffer_hashes_the_same() {
        let d = tmp("bigbuf");
        let f = d.join("big");
        let data: Vec<u8> = (0..(3 << 20)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&f, &data).unwrap();
        use sha2::{Digest, Sha256};
        let want: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(sha256_of(&f).unwrap(), want);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_path_is_not_an_error() {
        assert!(read(std::path::Path::new("Z:/없는파일.txt")).is_none());
    }

    #[test]
    fn a_missing_timestamp_renders_as_nothing() {
        assert_eq!(stamp(None), "");
        assert!(stamp(Some(std::time::SystemTime::now())).len() >= 19);
    }
}
