//! nabiTerm 벤더 패치 — SFTP v3 파일명 인코딩(원본 crate에는 없는 모듈).
//!
//! SFTP v3는 파일명 인코딩을 규정하지 않아 서버가 로컬 인코딩(CP949·Shift_JIS·GBK…)의
//! raw 바이트를 그대로 보낸다. 원본 crate는 모든 와이어 문자열을 `from_utf8_lossy`로
//! 디코드해 비UTF-8 한글이 U+FFFD로 **비가역 파괴**된다.
//!
//! ## 설계(v0.1.448 전면 개편 — 초판의 데이터 손실 결함 수정)
//!
//! 와이어 문자열은 파일명만이 아니다(핸들·오류 메시지·확장 이름도 같은 경로를 탄다).
//! 그래서 "무엇이 파일명인지" 모른 채 재인코딩하면 안 된다 — 대신 **왕복을 기억**한다:
//!
//! 1. **정확 복원**: 비UTF-8 바이트를 디코드하면 `원본 바이트`를 기억해 두고, 그 문자열을
//!    다시 보낼 때 **원본 바이트를 그대로 재생**한다. 덕분에 서버가 무엇을 쓰든(레거시
//!    인코딩 이름, 이진 핸들, 로컬라이즈된 오류 문자열) 왕복이 손실 없이 성립한다.
//!    유효 UTF-8은 애초에 손대지 않으므로 UTF-8 이름도 원본 그대로 나간다.
//! 2. **새 이름만 규약 적용**: 기억에 없는 문자열(사용자가 새로 만든 이름·업로드 대상)은
//!    감지/설정된 서버 인코딩으로 인코딩한다 — 그래야 서버·타 클라이언트가 바르게 읽는다.
//! 3. **감지는 파일명에서만 승격**: 자동 감지는 후보만 세우고, 파일명임이 확실한 지점
//!    (nabi-sftp `list()`의 readdir 응답)에서 [`promote_candidate`]로 확정한다. 이진 핸들이
//!    우연히 EUC-KR로 디코드돼 엉뚱한 인코딩이 각인되던 문제를 막는다.
//!
//! 남은 한계(문서화): 상태가 프로세스 전역이라 서로 다른 인코딩의 서버를 동시에 쓰면
//! **새 이름**에 마지막 감지가 쓰인다(기존 이름은 1의 정확 복원으로 무관).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// 파일명 인코딩 모드. Auto는 UTF-8 실패 시 레거시 인코딩을 점수로 골라 시도한다.
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
/// Auto에서 **확정된** 서버 인코딩(0=미확정) — 새 이름을 보낼 때 쓴다.
static DETECTED: AtomicU8 = AtomicU8::new(0);
/// 가장 최근 디코드가 제안한 인코딩(파일명 지점에서만 DETECTED로 승격).
static CANDIDATE: AtomicU8 = AtomicU8::new(0);

/// 디코드한 문자열 → 서버가 보낸 원본 바이트. 비UTF-8 문자열만 담는다(유효 UTF-8은
/// 그대로 재생되므로 기억할 필요가 없다). 상한을 넘으면 통째로 비운다 — 비면 규약
/// 인코딩으로 폴백할 뿐이라 안전하고, 실사용에서 도달하기 어려운 크기다.
static ORIGINAL: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);
const ORIGINAL_CAP: usize = 8192;

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

/// 전역 파일명 인코딩 모드를 설정한다(세션 접속·설정 저장 시). 감지 상태는 초기화.
pub fn set_filename_charset(c: Charset) {
    MODE_CELL.store(to_u8(c), Ordering::Relaxed);
    DETECTED.store(0, Ordering::Relaxed);
    CANDIDATE.store(0, Ordering::Relaxed);
}

/// 모드가 실제로 바뀔 때만 적용한다 — 매 프레임 호출해도 감지 상태를 리셋하지 않는다.
pub fn set_filename_charset_if_changed(c: Charset) {
    if from_u8(MODE_CELL.load(Ordering::Relaxed)) != c {
        set_filename_charset(c);
    }
}

/// **파일명임이 확실한 지점**에서 호출한다(readdir 응답 직후). 직전 디코드가 제안한
/// 인코딩을 확정으로 승격 — 핸들·오류 메시지 같은 비파일명 문자열이 감지를 오염시키지
/// 못하게 하는 장치다.
pub fn promote_candidate() {
    let c = CANDIDATE.load(Ordering::Relaxed);
    if c != 0 {
        DETECTED.store(c, Ordering::Relaxed);
    }
}

/// 현재 유효 인코딩(강제 모드=그 인코딩, Auto=확정된 것, 없으면 None=UTF-8).
fn effective() -> Option<&'static encoding_rs::Encoding> {
    match from_u8(MODE_CELL.load(Ordering::Relaxed)) {
        Charset::Utf8 => None,
        Charset::Auto => enc(from_u8(DETECTED.load(Ordering::Relaxed))),
        forced => enc(forced),
    }
}

/// Auto 모드에서 확정된 인코딩 라벨(UI 배지용). 미확정/비Auto면 None.
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

/// 디코드 결과가 그 인코딩의 "모국어 문자"를 얼마나 담고 있는지 점수화한다.
///
/// CP949의 트레일 바이트 범위가 넓어 일본어(CP932)·중국어(GBK) 바이트도 대개 EUC-KR로
/// **무손실 디코드된다** — 먼저 시도한 인코딩이 무조건 이기면 일본/중국 서버가 한글
/// 모지바케로 보인다. 그래서 순서가 아니라 점수로 고른다(가나=일본어 강신호, 한글=한국어).
fn script_score(c: Charset, s: &str) -> u32 {
    s.chars()
        .map(|ch| match (c, ch as u32) {
            (Charset::EucKr, 0xAC00..=0xD7A3) => 3, // 한글 음절
            (Charset::EucKr, 0x3130..=0x318F) => 3, // 호환 자모
            (Charset::ShiftJis, 0x3040..=0x30FF) => 3, // 히라가나·가타카나
            (Charset::ShiftJis, 0x4E00..=0x9FFF) => 1, // 한자(중국어와 공유)
            (Charset::Gbk, 0x4E00..=0x9FFF) => 1,
            (Charset::Gbk, 0xFF00..=0xFFEF) => 1, // 전각 기호
            _ => 0,
        })
        .sum()
}

/// 기억해 둔 원본 바이트를 등록한다(문자열 → 서버 원본).
fn remember(text: &str, bytes: &[u8]) {
    let mut g = ORIGINAL.lock().unwrap_or_else(|e| e.into_inner());
    let map = g.get_or_insert_with(HashMap::new);
    if map.len() >= ORIGINAL_CAP {
        map.clear();
    }
    map.insert(text.to_owned(), bytes.to_vec());
}

/// 기억한 원본 바이트를 찾는다.
fn recall(text: &str) -> Option<Vec<u8>> {
    let g = ORIGINAL.lock().unwrap_or_else(|e| e.into_inner());
    g.as_ref()?.get(text).cloned()
}

/// 와이어 바이트 → 문자열. 유효 UTF-8은 그대로, 아니면 모드에 따라 레거시 인코딩으로
/// 디코드하고 **원본 바이트를 기억**한다(정확 복원용). 실패해도 lossy 결과와 원본을
/// 함께 기억해 두므로, 표시가 깨지더라도 파일 조작(열기·삭제·이름변경)은 성립한다.
pub fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        // 서버가 UTF-8로 보낸 이름도 기억한다 — 레거시 인코딩이 감지된 서버에서도 이 이름은
        // UTF-8로 되돌려 보내야 찾힌다(UTF-8·CP949가 섞인 서버가 실제로 흔하다).
        // ASCII는 encode가 즉시 통과시키므로 기억할 필요가 없다.
        if !s.is_ascii() {
            remember(s, bytes);
        }
        return s.to_owned();
    }
    let mode = from_u8(MODE_CELL.load(Ordering::Relaxed));
    let decoded = match mode {
        Charset::Utf8 => None,
        Charset::Auto => {
            // 후보 전체를 디코드해 점수가 가장 높은 것을 고른다(동점이면 EUC-KR 우선).
            let mut best: Option<(u32, Charset, String)> = None;
            for c in [Charset::EucKr, Charset::ShiftJis, Charset::Gbk] {
                if let Some(s) = enc(c).and_then(|e| try_decode(e, bytes)) {
                    let sc = script_score(c, &s);
                    if best.as_ref().is_none_or(|(b, _, _)| sc > *b) {
                        best = Some((sc, c, s));
                    }
                }
            }
            best.map(|(_, c, s)| {
                // 확정이 아니라 '후보'만 세운다 — 파일명 지점에서 promote_candidate로 승격.
                CANDIDATE.store(to_u8(c), Ordering::Relaxed);
                s
            })
        }
        forced => enc(forced).and_then(|e| try_decode(e, bytes)),
    };
    let text = decoded.unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned());
    remember(&text, bytes);
    text
}

/// 문자열 → 와이어 바이트. 서버에서 받은 문자열은 **원본 바이트를 정확히 재생**하고,
/// 기억에 없는 새 이름만 서버 규약(감지/설정 인코딩)으로 인코딩한다.
pub fn encode(s: &str) -> std::borrow::Cow<'_, [u8]> {
    if s.is_ascii() {
        return std::borrow::Cow::Borrowed(s.as_bytes());
    }
    if let Some(orig) = recall(s) {
        return std::borrow::Cow::Owned(orig); // 서버가 보낸 그 바이트 그대로.
    }
    match effective() {
        None => std::borrow::Cow::Borrowed(s.as_bytes()),
        Some(e) => {
            let (b, _, had_errors) = e.encode(s);
            if had_errors {
                std::borrow::Cow::Borrowed(s.as_bytes()) // 인코딩 불가 문자는 UTF-8로.
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

    fn sjis(s: &str) -> Vec<u8> {
        encoding_rs::SHIFT_JIS.encode(s).0.into_owned()
    }

    /// 전역 상태를 공유하므로 테스트를 직렬화한다.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static M: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn auto_roundtrips_cp949_names() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        let wire = cp949("한글파일.txt");
        assert!(std::str::from_utf8(&wire).is_err());
        let name = decode(&wire);
        assert_eq!(name, "한글파일.txt");
        promote_candidate();
        assert_eq!(encode(&name).as_ref(), wire.as_slice(), "원본 바이트 정확 재생");
        assert_eq!(detected_label(), Some("EUC-KR"));
    }

    /// 초판 최악 결함: CP949 감지 후 **UTF-8 이름 파일이 열리지 않던** 회귀.
    #[test]
    fn utf8_names_survive_after_legacy_detection() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        decode(&cp949("보고서.hwp")); // 레거시 이름 → EUC-KR 후보
        promote_candidate();
        let utf8_name = "계약서.pdf";
        assert_eq!(decode(utf8_name.as_bytes()), utf8_name);
        assert_eq!(
            encode(utf8_name).as_ref(),
            utf8_name.as_bytes(),
            "UTF-8 이름은 UTF-8로 나가야 서버에서 찾힌다"
        );
    }

    /// 이진 핸들(비UTF-8)은 그대로 재생되고, 감지를 오염시키지 않는다.
    #[test]
    fn opaque_handles_replay_exactly_and_do_not_latch() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        let handle = vec![0xB0u8, 0xA1, 0x00, 0xFF, 0x01];
        let s = decode(&handle);
        assert_eq!(encode(&s).as_ref(), handle.as_slice(), "핸들 바이트 정확 재생");
        assert_eq!(detected_label(), None, "승격 없이는 감지 확정 안 됨(핸들 오염 방지)");
    }

    /// 일본어 서버가 한글로 오판되지 않는다(EUC-KR도 무손실 디코드되지만 점수로 구분).
    #[test]
    fn japanese_names_are_not_misdetected_as_korean() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        let wire = sjis("テスト.txt");
        assert!(encoding_rs::EUC_KR.decode(&wire).2 == false, "EUC-KR로도 디코드되는 표본이어야 의미 있다");
        let name = decode(&wire);
        promote_candidate();
        assert_eq!(name, "テスト.txt");
        assert_eq!(detected_label(), Some("Shift_JIS"));
        assert_eq!(encode(&name).as_ref(), wire.as_slice());
    }

    /// Shift_JIS 왕복 비대칭 문자(NEC/IBM 중복)도 원본 재생으로 살아난다.
    #[test]
    fn lossy_encodings_still_roundtrip_via_memory() {
        let _g = lock();
        set_filename_charset(Charset::ShiftJis);
        let wire = vec![0x87, 0x90, 0x2E, 0x74, 0x78, 0x74]; // '≒.txt' (NEC 행13)
        let name = decode(&wire);
        assert_eq!(encode(&name).as_ref(), wire.as_slice(), "재인코딩이 아니라 원본 재생");
        set_filename_charset(Charset::Auto);
    }

    #[test]
    fn utf8_mode_keeps_lossy_behavior_and_ascii_passthrough() {
        let _g = lock();
        set_filename_charset(Charset::Utf8);
        assert!(decode(&cp949("한글")).contains('\u{FFFD}'));
        assert_eq!(encode("abc.txt").as_ref(), b"abc.txt");
        set_filename_charset(Charset::Auto);
    }

    #[test]
    fn new_names_use_server_convention() {
        let _g = lock();
        set_filename_charset(Charset::Auto);
        decode(&cp949("기존파일.txt"));
        promote_candidate();
        // 기억에 없는 새 이름 = 사용자가 방금 만든 이름 → 서버 규약(CP949)으로 보낸다.
        assert_eq!(encode("새파일.txt").as_ref(), cp949("새파일.txt").as_slice());
    }
}
