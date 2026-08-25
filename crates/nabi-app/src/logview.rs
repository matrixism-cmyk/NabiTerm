//! 진단 로그 **앱 안에서 보기** — 원격 지원 때 "폴더 열어 파일 찾아 보내 주세요"를 없앤다.
//!
//! 지금까지는 탐색기만 열어 줬다(`helppages`). 문제가 난 사용자에게 그건 또 하나의 숙제다 —
//! 어느 파일인지, 어디부터가 문제인지 스스로 골라야 한다.
//!
//! 여기서는 최근 로그를 바로 보여 주고, **오류 줄만 보기**와 **한 번에 복사**를 준다.
//! 지원 요청에 붙일 것을 사용자가 판단하지 않아도 되게 하는 것이 요점이다.
//!
//! 로그는 커질 수 있으므로 **뒤에서부터** 정해진 양만 읽는다. 사후 진단에서 필요한 것은
//! 대개 마지막 부분이다.

use std::path::Path;

/// 화면에 들일 최대 바이트(뒤에서부터). 이보다 크면 앞을 자른다.
const TAIL_BYTES: u64 = 512 * 1024;

/// 로그 한 덩어리.
pub(crate) struct LogText {
    pub file: String,
    pub body: String,
    /// 앞을 잘라 냈는가(그러면 화면에 그 사실을 알린다).
    pub truncated: bool,
}

/// 로그 폴더에서 가장 최근 파일을 뒤에서부터 읽는다.
pub(crate) fn latest(dir: &Path) -> Option<LogText> {
    let newest = newest_file(dir)?;
    let file = newest.file_name()?.to_string_lossy().into_owned();
    let (body, truncated) = read_tail(&newest, TAIL_BYTES)?;
    Some(LogText { file, body, truncated })
}

/// 폴더에서 가장 최근에 고쳐진 파일.
fn newest_file(dir: &Path) -> Option<std::path::PathBuf> {
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let Ok(md) = e.metadata() else { continue };
        if !md.is_file() {
            continue;
        }
        let Ok(t) = md.modified() else { continue };
        if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
            best = Some((t, e.path()));
        }
    }
    best.map(|(_, p)| p)
}

/// 파일 끝에서 `max` 바이트를 읽는다. 잘랐으면 두 번째 값이 true.
///
/// 글자 한가운데서 자르면 깨지므로, 자른 뒤 **첫 줄바꿈까지 버린다** — 어차피 반쪽 줄이다.
fn read_tail(path: &Path, max: u64) -> Option<(String, bool)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let cut = len > max;
    if cut {
        f.seek(SeekFrom::Start(len - max)).ok()?;
    }
    let mut buf = Vec::with_capacity(max.min(len) as usize);
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let body = if cut {
        text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or(text)
    } else {
        text
    };
    Some((body, cut))
}

/// 오류·경고로 보이는 줄만 남긴다.
///
/// 지원 요청에 붙일 것은 대개 이 줄들이다. 판정은 우리 로그 형식(tracing)의 수준 표시와
/// 패닉 문구를 본다 — 사용자가 무엇이 중요한지 고르지 않아도 되게.
pub(crate) fn only_problems(body: &str) -> String {
    body.lines()
        .filter(|l| l.contains("ERROR") || l.contains(" WARN") || l.contains("panicked"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("nabi-logview-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn it_shows_the_most_recent_log_file() {
        let d = dir("newest");
        std::fs::write(d.join("old.log"), "old line\n").unwrap();
        // 수정 시각이 확실히 뒤가 되도록 파일을 다시 쓴다.
        std::fs::write(d.join("new.log"), "new line\n").unwrap();
        let got = latest(&d).expect("파일이 있으면 읽어야 한다");
        assert_eq!(got.body, "new line\n");
        assert_eq!(got.file, "new.log");
        assert!(!got.truncated);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 큰 로그는 **뒤에서부터** 읽는다 — 사후 진단에 필요한 것은 마지막 부분이다.
    #[test]
    fn a_huge_log_is_read_from_the_end() {
        let d = dir("tail");
        let p = d.join("big.log");
        let mut text = "머리부분 버려질 줄\n".repeat(60_000);
        text.push_str("마지막 표식\n");
        std::fs::write(&p, &text).unwrap();
        let (body, cut) = read_tail(&p, 1024).unwrap();
        assert!(cut, "잘렸다고 알려야 한다");
        assert!(body.len() < 1200, "뒤쪽만 읽어야 한다: {}", body.len());
        assert!(body.ends_with("마지막 표식\n"));
        assert!(!body.contains("\u{fffd}"), "글자 한가운데서 잘려 깨지면 안 된다");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_problems_keeps_errors_and_panics() {
        let body = "2026-01-01 INFO started\n2026-01-01 ERROR bad thing\nplain line\nthread panicked at x\n2026 WARN hmm\n";
        let got = only_problems(body);
        assert!(got.contains("ERROR bad thing"));
        assert!(got.contains("panicked"));
        assert!(got.contains("WARN hmm"));
        assert!(!got.contains("started"), "정보 줄은 빠져야 한다");
        assert!(!got.contains("plain line"));
    }

    #[test]
    fn an_empty_folder_is_not_an_error() {
        assert!(latest(&dir("empty")).is_none());
        assert!(latest(std::path::Path::new("Z:/없는폴더")).is_none());
    }
}
