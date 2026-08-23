//! nabiPad HEX(이진) 편집 버퍼. 이진 파일 자동 감지 + 텍스트↔HEX 전환에 쓰인다.
//! 렌더/입력은 [`crate::edithexview`], 바이트 저장은 [`crate::hexdata`].
//!
//! 예전에는 파일 전체를 `Vec<u8>`로 올려서 16MB가 넘으면 편집을 막고 읽기 전용 뷰어로
//! 떨어뜨렸다(사용자 보고 2026-08-22). 지금은 원본을 mmap해 두고 편집만 조각으로 쌓으므로
//! **크기 제한이 없다** — 메모리는 파일 크기가 아니라 편집량에 비례한다.

/// 한 줄에 표시할 바이트 수(고정 폭 레이아웃 기준).
pub const COLS: usize = 16;

/// 이진 편집 버퍼. 덮어쓰기/삽입 두 모드, 블록 선택·복사/붙여넣기·삽입/삭제 지원.
pub struct HexBuf {
    /// 바이트 본체 — mmap 원본 + 편집 조각(hexdata). 통째로 RAM에 올리지 않는다.
    pub data: crate::hexdata::HexData,
    /// 현재 바이트 위치.
    pub cursor: usize,
    /// 선택 시작(있으면 [min(anchor,cursor), max+1) 가 선택 범위).
    pub anchor: Option<usize>,
    /// HEX 칼럼에서 하위 니블 입력 차례면 true.
    pub low_nibble: bool,
    /// 입력 포커스가 ASCII 칼럼이면 true(아니면 HEX 칼럼).
    pub ascii: bool,
    /// 삽입 모드(true=삽입, false=덮어쓰기 기본).
    pub insert_mode: bool,
    pub dirty: bool,
    /// 오프셋 이동 입력 버퍼 + 스크롤 대상 줄(소비되면 None).
    pub goto: String,
    pub scroll_to: Option<usize>,
    /// 바이트 검색 입력 + 모드(true=ASCII 문자열, false=16진 패턴) + 바꿀 값.
    pub find: String,
    pub find_text: bool,
    pub replace: String,
    /// 실행 취소/다시 실행 스택. 누적 크기는 MAX_UNDO_BYTES로 제한.
    pub undo: Vec<HexDelta>,
    pub redo: Vec<HexDelta>,
}

/// 되돌릴 수 있는 한 번의 변경 — `bytes[at..at+old.len()]`이 `new`로 바뀌었다.
///
/// 전체 스냅샷 대신 바뀐 구간만 담는다. 100MB 파일에서 한 바이트를 고칠 때마다
/// 100MB를 복제하던 것이 없어져, 편집 한 번의 비용이 파일 크기와 무관해진다.
pub struct HexDelta {
    pub at: usize,
    pub old: Vec<u8>,
    pub new: Vec<u8>,
    /// 변경 직전 커서(되돌릴 때 복원).
    pub cur: usize,
    /// **앞 기록과 한 단위**인가. "모두 바꾸기"처럼 한 번의 조작이 여러 자리를 고칠 때,
    /// 취소 한 번으로 전부 되돌아가야 한다(고친 횟수만큼 눌러야 하면 쓸 수 없다).
    pub cont: bool,
}

/// undo 히스토리 누적 바이트 상한(초과 시 오래된 기록부터 버림).
pub const MAX_UNDO_BYTES: usize = 64_000_000;

impl HexBuf {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self::from_data(crate::hexdata::HexData::from_vec(bytes))
    }

    /// 이미 만들어 둔 바이트 본체로 버퍼를 만든다(파일 매핑 경로).
    pub fn from_data(data: crate::hexdata::HexData) -> Self {
        Self {
            data, cursor: 0, anchor: None, low_nibble: false, ascii: false,
            insert_mode: false, dirty: false, goto: String::new(), scroll_to: None,
            find: String::new(), find_text: false, replace: String::new(), undo: Vec::new(), redo: Vec::new(),
        }
    }

    /// 오프셋 입력으로 커서를 옮기고 그 줄로 스크롤한다. `0x`=16진/그 외=10진,
    /// `+N`·`-N`이면 현재 커서 기준 상대 이동(구조체 탐색에 편리).
    pub fn jump_to_offset(&mut self) {
        let s = self.goto.trim();
        let target = if let Some(rest) = s.strip_prefix('+') {
            parse_offset(rest).map(|d| self.cursor.saturating_add(d))
        } else if let Some(rest) = s.strip_prefix('-') {
            parse_offset(rest).map(|d| self.cursor.saturating_sub(d))
        } else {
            parse_offset(s)
        };
        if let Some(off) = target {
            self.cursor = off.min(self.data.len().saturating_sub(1));
            self.scroll_to = Some(self.cursor / COLS);
            self.low_nibble = false;
            self.anchor = None;
        }
    }

    /// 선택 범위 [lo, hi)(바이트). 선택이 없으면 None.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        Some((a.min(self.cursor), a.max(self.cursor) + 1))
    }

    /// 이동 전 선택 앵커를 갱신한다(select면 시작 고정, 아니면 해제).
    fn update_anchor(&mut self, select: bool) {
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
    }

    /// 파일을 HEX 버퍼로 연다. **크기 제한이 없다** — 원본은 매핑만 하고 읽지 않는다.
    pub fn open(path: &std::path::Path) -> Option<Self> {
        crate::hexdata::HexData::map_file(path).ok().map(Self::from_data)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 한 바이트(범위 밖이면 None).
    pub fn at(&self, i: usize) -> Option<u8> {
        self.data.get(i)
    }

    /// `[lo, hi)` 구간 복사 — 화면 한 페이지·검사기처럼 **작은 구간**에만 쓴다.
    pub fn range(&self, lo: usize, hi: usize) -> Vec<u8> {
        self.data.read(lo, hi.saturating_sub(lo))
    }

    /// 전체를 한 벌로 복사한다. 큰 파일에서는 그만큼 메모리를 쓰므로 시험·작은 문서 전용.
    pub fn bytes(&self) -> Vec<u8> {
        self.data.to_vec()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 표시 줄 수(마지막 부분 줄 포함).
    pub fn rows(&self) -> usize {
        self.data.len().div_ceil(COLS).max(1)
    }

    /// 커서를 delta만큼 이동(경계 클램프). select면 선택 확장. 이동하면 니블 상태 초기화.
    pub fn move_by(&mut self, delta: isize, select: bool) {
        self.update_anchor(select);
        let last = self.data.len().saturating_sub(1);
        self.cursor = (self.cursor as isize + delta).clamp(0, last as isize) as usize;
        self.low_nibble = false;
    }

    /// 줄 시작/끝으로(Home/End). select면 선택 확장.
    pub fn line_edge(&mut self, end: bool, select: bool) {
        self.update_anchor(select);
        let start = (self.cursor / COLS) * COLS;
        let stop = (start + COLS - 1).min(self.data.len().saturating_sub(1));
        self.cursor = if end { stop } else { start };
        self.low_nibble = false;
    }

    /// 데이터 인스펙터: 커서 위치 바이트를 u8/u16/u32(리틀엔디언) 정수로 해석한다(바이트 부족 시 None).
    pub fn inspect(&self) -> (Option<u8>, Option<u16>, Option<u32>) {
        // 커서 앞 8바이트만 한 번 읽어 전부 여기서 해석한다(조각 표를 여러 번 훑지 않는다).
        let w = self.data.read(self.cursor, 8);
        let b = w.first().copied();
        let u16v = (w.len() >= 2).then(|| u16::from_le_bytes([w[0], w[1]]));
        let u32v = (w.len() >= 4).then(|| u32::from_le_bytes([w[0], w[1], w[2], w[3]]));
        (b, u16v, u32v)
    }

    /// 데이터 인스펙터: (상태바 간략 문자열, 호버 상세 문자열). 커서에 바이트가 없으면 None.
    /// 간략 = "u8 N · u16 N · u32 N"(LE), 상세 = 부호 정수·16진·2진까지(HxD식).
    pub fn inspector(&self) -> Option<(String, String)> {
        let (b, u16o, u32o) = self.inspect();
        let b = b?;
        let u16s = u16o.map(|v| format!(" \u{00b7} u16 {v}")).unwrap_or_default();
        let u32s = u32o.map(|v| format!(" \u{00b7} u32 {v}")).unwrap_or_default();
        let brief = format!("u8 {b}{u16s}{u32s}");
        let mut detail = format!("u8 {b}   i8 {}   0x{b:02X}   0b{b:08b}", b as i8);
        if let Some(v) = u16o {
            detail.push_str(&format!("\nu16 {v}   i16 {}   BE {}", v as i16, v.swap_bytes()));
        }
        if let Some(v) = u32o {
            // 부동소수(IEEE 754 f32)·빅엔디언까지 — 바이너리/파일 포맷 분석용(HxD식).
            detail.push_str(&format!("\nu32 {v}   i32 {}   f32 {}   BE {}", v as i32, f32::from_bits(v), v.swap_bytes()));
            // Unix epoch(time_t, 초) 해석 — 바이너리 속 타임스탬프 식별.
            if let Some(dt) = chrono::DateTime::from_timestamp(v as i64, 0) {
                detail.push_str(&format!("\ntime_t {} UTC", dt.format("%Y-%m-%d %H:%M:%S")));
            }
        }
        // 커서부터 24바이트만 읽어 u64와 ASCII 런을 함께 본다(조각 표를 한 번만 훑는다).
        let tail = self.data.read(self.cursor, 24);
        if let Ok(arr) = <[u8; 8]>::try_from(&tail[..8.min(tail.len())]) {
            let u = u64::from_le_bytes(arr);
            detail.push_str(&format!("\nu64 {u}   i64 {}   f64 {}", u as i64, f64::from_le_bytes(arr)));
        }
        // 커서부터 이어지는 인쇄 가능한 ASCII 문자열(바이너리 속 문자열 식별).
        let s = ascii_run(&tail, 0, 24);
        if !s.is_empty() {
            detail.push_str(&format!("\nstr \"{s}\""));
        }
        Some((brief, detail))
    }

    /// 전체 선택.
    pub fn select_all(&mut self) {
        if !self.data.is_empty() {
            self.anchor = Some(0);
            self.cursor = self.data.len() - 1;
        }
    }
}

/// 커서부터 인쇄 가능한 ASCII(0x20~0x7E) 런(최대 max자, 비인쇄/NUL에서 중단).
fn ascii_run(bytes: &[u8], at: usize, max: usize) -> String {
    bytes.iter().skip(at).take(max).take_while(|&&b| (0x20..0x7f).contains(&b)).map(|&b| b as char).collect()
}

/// 오프셋 문자열 → 바이트 위치. `0x`/`0X` 접두는 16진, 그 외는 10진. 실패 시 None.
fn parse_offset(s: &str) -> Option<usize> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => usize::from_str_radix(hex, 16).ok(),
        None => s.parse::<usize>().ok(),
    }
}

/// 파일 앞부분(최대 8KB)만 읽어 이진 여부를 추정한다(대용량도 빠르게 판정).
pub fn peek_is_binary(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut buf = [0u8; 8192];
    match f.read(&mut buf) {
        Ok(n) => is_binary(&buf[..n]),
        Err(_) => false,
    }
}

/// 앞부분 표본으로 이진 파일 여부를 추정한다.
///
/// 예전에는 `NUL이 있으면 이진`이었는데, **UTF-16 텍스트가 여기 걸렸다**. UTF-16LE는
/// ASCII를 `41 00`으로 저장하니 NUL이 널려 있다. BOM이 붙어 있고 우리 디코더가 정상
/// 처리하는데도 그 앞에서 이진으로 판정돼 HEX로 열렸다(사용자 지적 2026-08-22).
///
/// 그래서 순서를 바꿨다: ① BOM이 텍스트 인코딩이면 텍스트, ② BOM이 없어도 NUL이
/// **한쪽 바이트에만 몰려 있으면** UTF-16으로 보고 텍스트, ③ 그 외에 NUL이 있으면 이진.
pub fn is_binary(bytes: &[u8]) -> bool {
    let n = bytes.len().min(8192);
    if n == 0 {
        return false;
    }
    let sample = &bytes[..n];
    if has_text_bom(sample) || looks_like_utf16(sample) {
        return false;
    }
    if sample.contains(&0) {
        return true; // NUL 바이트 → 이진.
    }
    // 탭(09)·개행(0A)·캐리지(0D) 외 제어문자 비율이 높으면 이진.
    let ctrl = sample.iter().filter(|&&b| (b < 0x20 && !matches!(b, 0x09 | 0x0A | 0x0D)) || b == 0x7F).count();
    ctrl * 100 / n > 30
}

/// 텍스트 인코딩의 BOM인가(UTF-8 / UTF-16 LE·BE / UTF-32 LE·BE).
/// UTF-32 판정을 UTF-16보다 먼저 해야 한다 — `FF FE 00 00`은 둘 다처럼 보인다.
fn has_text_bom(b: &[u8]) -> bool {
    b.starts_with(&[0xEF, 0xBB, 0xBF])
        || b.starts_with(&[0xFF, 0xFE, 0x00, 0x00])
        || b.starts_with(&[0x00, 0x00, 0xFE, 0xFF])
        || b.starts_with(&[0xFF, 0xFE])
        || b.starts_with(&[0xFE, 0xFF])
}

/// BOM 없는 UTF-16 추정 — NUL이 **짝수 자리에만** 또는 **홀수 자리에만** 몰려 있는가.
///
/// UTF-16LE에서 ASCII는 `41 00`(높은 바이트가 0), 한글은 `00 AC`처럼 낮은 바이트가 0인
/// 경우가 섞인다. 어느 쪽이든 NUL은 **한쪽 정렬 위치로 몰린다.** 반대로 진짜 이진 파일은
/// `00 00`이 흔하고 NUL이 양쪽에 고루 나온다.
fn looks_like_utf16(s: &[u8]) -> bool {
    let n = s.len() & !1; // 짝수 길이로 자른다(쌍으로 본다).
    if n < 16 {
        return false; // 표본이 너무 작으면 판단하지 않는다(미번역이 오판보다 낫다).
    }
    let (mut even, mut odd, mut both) = (0usize, 0usize, 0usize);
    for i in (0..n).step_by(2) {
        let (a, b) = (s[i] == 0, s[i + 1] == 0);
        if a && b {
            both += 1; // `00 00` — 이진 쪽 신호.
        }
        even += usize::from(a);
        odd += usize::from(b);
    }
    if both > 0 || even + odd < 4 {
        return false;
    }
    // 한쪽이 다른 쪽보다 압도적으로 많아야 한다(8배). 애매하면 이진으로 둔다.
    let (hi, lo) = (even.max(odd), even.min(odd));
    lo * 8 < hi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_goto_moves_cursor() {
        let mut h = HexBuf::from_bytes(vec![0u8; 100]);
        h.cursor = 0x20;
        h.goto = "+0x10".into();
        h.jump_to_offset();
        assert_eq!(h.cursor, 0x30); // 상대 +16진.
        h.goto = "-16".into();
        h.jump_to_offset();
        assert_eq!(h.cursor, 0x20); // 상대 -10진.
        h.goto = "0x05".into();
        h.jump_to_offset();
        assert_eq!(h.cursor, 0x05); // 절대.
    }

    #[test]
    fn parses_offsets() {
        assert_eq!(parse_offset("0x10"), Some(16));
        assert_eq!(parse_offset("255"), Some(255));
        assert_eq!(parse_offset(" 0XFF "), Some(255));
        assert_eq!(parse_offset("zz"), None);
    }

    #[test]
    fn detects_binary_and_text() {
        assert!(is_binary(&[0x00, 0x01, 0x02]));
        assert!(!is_binary(b"hello\nworld\t!"));
        assert!(!is_binary(&[]));
    }

    #[test]
    fn inspector_shows_float_and_64bit() {
        // f32 LE 0x3F800000 = 1.0, 8바이트 있으면 u64/f64 줄도 표시.
        let h = HexBuf::from_bytes(vec![0, 0, 0x80, 0x3F, 0, 0, 0, 0]);
        let (_, detail) = h.inspector().unwrap();
        assert!(detail.contains("f32 1"), "{detail}");
        assert!(detail.contains("u64 "), "{detail}");
        assert!(detail.contains("f64 "), "{detail}");
        // u32=0 → time_t 1970-01-01.
        let z = HexBuf::from_bytes(vec![0, 0, 0, 0]);
        assert!(z.inspector().unwrap().1.contains("time_t 1970-01-01"));
        // 커서부터 ASCII 문자열 식별.
        assert_eq!(super::ascii_run(b"Hi\x00x", 0, 24), "Hi");
        assert_eq!(super::ascii_run(b"\x01ab", 0, 24), ""); // 시작이 비인쇄.
    }

    #[test]
    fn nibble_then_ascii_overwrite() {
        let mut h = HexBuf::from_bytes(vec![0x00, 0xFF]);
        h.input_nibble(0xA); // 상위
        h.input_nibble(0xB); // 하위 → 0xAB, 커서 1로 진행
        assert_eq!(h.at(0), Some(0xAB));
        assert_eq!(h.cursor, 1);
        h.input_ascii(b'Z');
        assert_eq!(h.at(1), Some(b'Z'));
        assert!(h.dirty);
    }

    /// UTF-16 텍스트를 이진으로 오판하면 HEX로 열려 편집이 막힌다(사용자 지적 2026-08-22).
    #[test]
    fn utf16_text_is_not_binary() {
        let msg = "The quick brown fox, 안녕하세요 반갑습니다";
        let mut le = vec![0xFF, 0xFE];
        for c in msg.encode_utf16() {
            le.extend_from_slice(&c.to_le_bytes());
        }
        assert!(!is_binary(&le), "BOM 있는 UTF-16LE는 텍스트다");

        let mut be = vec![0xFE, 0xFF];
        for c in msg.encode_utf16() {
            be.extend_from_slice(&c.to_be_bytes());
        }
        assert!(!is_binary(&be), "BOM 있는 UTF-16BE는 텍스트다");

        let mut nb = Vec::new();
        for c in "The quick brown fox jumps over the lazy dog".encode_utf16() {
            nb.extend_from_slice(&c.to_le_bytes());
        }
        assert!(!is_binary(&nb), "BOM이 없어도 한쪽에 몰린 NUL은 UTF-16이다");
    }

    /// 진짜 이진은 여전히 이진이어야 한다 — 위 완화가 구멍이 되면 안 된다.
    #[test]
    fn real_binaries_are_still_binary() {
        let mut exe = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xff\xff\x00\x00".to_vec();
        exe.extend_from_slice(&[0u8; 64]);
        assert!(is_binary(&exe), "실행 파일은 이진이다");

        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&[0u8; 32]);
        assert!(is_binary(&png), "PNG는 이진이다");

        assert!(is_binary(&[0u8; 256]), "0으로 채운 파일은 이진이다");
    }

}
