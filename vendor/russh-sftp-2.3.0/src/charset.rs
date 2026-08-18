//! nabiTerm 벤더 패치 — SFTP v3 파일명 인코딩 지원(원본 crate에는 없는 모듈).
//!
//! SFTP v3는 파일명 인코딩을 규정하지 않아 서버가 로컬 인코딩(CP949·Shift_JIS·GBK…)의
//! raw 바이트를 그대로 보낸다. 원본 crate는 모든 와이어 문자열을 `from_utf8_lossy`로
//! 디코드해 비UTF-8 한글이 U+FFFD로 **비가역 파괴**된다. 이 모듈은 wire 문자열의
//! 디코드(`buf.rs`)·인코드(`ser.rs`) 단일 지점에 끼어들어 왕복을 살린다.
//!
//! 프로세스 전역 설정이다(세션별 아님 — 디코드가 백그라운드 읽기 루프에서 일어나
//! 세션 문맥이 없다). 서로 다른 인코딩의 서버를 "동시에" 쓰는 경우는 마지막
//! 설정/감지가 공유되는 한계가 있다(문서화된 제한).

use std::sync::atomic::{AtomicU8, Ordering};

/// 파일명 인코딩 모드. Auto는 UTF-8 실패 시 레거시 인코딩을 순서대로 시도한다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Charset {
    Utf8,
    Auto,
    EucKr,
    ShiftJis,
    Gbk,
}

/// 전역 모드(기본 1=Auto).
static MODE_CELL: AtomicU8 = AtomicU8::new(1);
/// Auto 모드에서 실제로 감지된 서버 인코딩(0=미감지).
static DETECTED: AtomicU8 = AtomicU8::new(0);

fn to_u8(c: Charset) -> u8 {
    match c {
        Charset::Utf8 => 0,
        Charset::Auto => 1,
        Charset::EucKr => 2,
        Charset::ShiftJis => 3,
        Charset::Gbk => 4,
    }
}

fn from_u8(v: u8) -> Charset {
    match v {
        0 => Charset::Utf8,
        2 => Charset::EucKr,
        3 => Charset::ShiftJis,
        4 => Charset::Gbk,
        _ => Charset::Auto,
    }
}

fn enc(c: Charset) -> Option<&'static encoding_rs::Encoding> {
    match c {
        // encoding_rs의 "EUC-KR"은 실제로 CP949 상위호환(웹 표준 매핑).
        Charset::EucKr => Some(encoding_rs::EUC_KR),
        Charset::ShiftJis => Some(encoding_rs::SHIFT_JIS),
        Charset::Gbk => Some(encoding_rs::GBK),
        _ => None,
    }
}

/// 전역 파일명 인코딩 모드를 설정한다(세션 접속 시 호출). Auto로 바꾸면 감지 상태 초기화.
pub fn set_filename_charset(c: Charset) {
    MODE_CELL.store(to_u8(c), Ordering::Relaxed);
    DETECTED.store(0, Ordering::Relaxed);
}

/// 모드가 실제로 바뀔 때만 적용한다 — 매 프레임 호출해도 Auto 감지 상태를 리셋하지 않는다.
pub fn set_filename_charset_if_changed(c: Charset) {
    if from_u8(MODE_CELL.load(Ordering::Relaxed)) != c {
        set_filename_charset(c);
    }
}

/// 현재 유효 인코딩(강제 모드=그 인코딩, Auto=감지된 것 또는 None=UTF-8).
fn effective() -> Option<&'static encoding_rs::Encoding> {
    match from_u8(MODE_CELL.load(Ordering::Relaxed)) {
        Charset::Utf8 => None,
        Charset::Auto => enc(from_u8(DETECTED.load(Ordering::Relaxed))),
        forced => enc(forced),
    }
}

/// Auto 모드에서 감지된 인코딩 라벨(UI 표시용). 미감지/비Auto면 None.
pub fn detected_label() -> Option<&'static str> {
    match from_u8(DETECTED.load(Ordering::Relaxed)) {
        Charset::EucKr => Some("EUC-KR"),
        Charset::ShiftJis => Some("Shift_JIS"),
        Charset::Gbk => Some("GBK"),
        _ => None,
    }
}

/// 무손실 디코드 시도 — 치환문자 없이 완전 디코드될 때만 Some.
fn try_decode(e: &'static encoding_rs::Encoding, b: &[u8]) -> Option<String> {
    let (s, _, had_errors) = e.decode(b);
    (!had_errors).then(|| s.into_owned())
}

/// 와이어 바이트 → 문자열. UTF-8 우선, 실패 시 모드에 따라 레거시 인코딩 폴백.
/// (pub: nabi-sftp가 워크스페이스 테스트에서 패치 동작을 검증하는 데 쓴다.)
pub fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    match from_u8(MODE_CELL.load(Ordering::Relaxed)) {
        Charset::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Charset::Auto => {
            // 감지된 인코딩 우선, 아니면 CP949→Shift_JIS→GBK 순서로 무손실 판정.
            if let Some(e) = enc(from_u8(DETECTED.load(Ordering::Relaxed))) {
                if let Some(s) = try_decode(e, bytes) {
                    return s;
                }
            }
            for c in [Charset::EucKr, Charset::ShiftJis, Charset::Gbk] {
                if let Some(s) = enc(c).and_then(|e| try_decode(e, bytes)) {
                    DETECTED.store(to_u8(c), Ordering::Relaxed);
                    return s;
                }
            }
            String::from_utf8_lossy(bytes).into_owned()
        }
        forced => enc(forced)
            .and_then(|e| try_decode(e, bytes))
            .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned()),
    }
}

/// 문자열 → 와이어 바이트. ASCII/UTF-8 모드는 그대로, 레거시 인코딩이 유효하면 무손실
/// 재인코딩(서버 파일시스템의 원래 바이트로 복원). 인코딩 불가 문자는 UTF-8 폴백.
pub(crate) fn encode(s: &str) -> std::borrow::Cow<'_, [u8]> {
    if s.is_ascii() {
        return std::borrow::Cow::Borrowed(s.as_bytes());
    }
    match effective() {
        None => std::borrow::Cow::Borrowed(s.as_bytes()),
        Some(e) => {
            let (b, _, had_errors) = e.encode(s);
            if had_errors {
                std::borrow::Cow::Borrowed(s.as_bytes())
            } else {
                std::borrow::Cow::Owned(b.into_owned())
            }
        }
    }
}

/// 외부(nabi-sftp 확장 페이로드 등)에서 같은 규칙으로 경로를 인코딩할 때 쓴다.
pub fn encode_path(s: &str) -> Vec<u8> {
    encode(s).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp949(s: &str) -> Vec<u8> {
        encoding_rs::EUC_KR.encode(s).0.into_owned()
    }

    /// 전역 모드를 공유하므로 테스트를 직렬화한다(병렬 실행 시 경합 방지).
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static M: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn auto_roundtrips_cp949_names() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        let wire = cp949("한글파일.txt");
        assert!(std::str::from_utf8(&wire).is_err(), "CP949 한글은 유효 UTF-8이 아니어야 테스트가 의미 있다");
        let name = decode(&wire);
        assert_eq!(name, "한글파일.txt");
        assert_eq!(encode(&name).as_ref(), wire.as_slice(), "송신 시 원래 서버 바이트로 복원");
        assert_eq!(detected_label(), Some("EUC-KR"));
    }

    #[test]
    fn utf8_mode_keeps_legacy_behavior_and_ascii_passthrough() {
        let _g = lock();
        set_filename_charset(Charset::Utf8);
        let wire = cp949("한글");
        assert!(decode(&wire).contains('\u{FFFD}'), "UTF-8 강제 모드는 기존과 동일(lossy)");
        assert_eq!(encode("abc.txt").as_ref(), b"abc.txt");
        set_filename_charset(Charset::Auto);
    }

    #[test]
    fn forced_euckr_decodes_and_encodes() {
        let _g = lock();
        set_filename_charset(Charset::EucKr);
        let wire = cp949("보고서 최종.hwp");
        assert_eq!(decode(&wire), "보고서 최종.hwp");
        assert_eq!(encode("보고서 최종.hwp").as_ref(), wire.as_slice());
        set_filename_charset(Charset::Auto);
    }

    #[test]
    fn valid_utf8_never_touched() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        assert_eq!(decode("한글.txt".as_bytes()), "한글.txt", "유효 UTF-8은 폴백을 타지 않는다");
    }
}
