//! nabiPad HEX(이진) 편집 버퍼 — 바이트 배열을 통째로 RAM에 적재해 덮어쓰기 편집한다.
//! 이진 파일 자동 감지 + 텍스트↔HEX 전환에 쓰인다. 렌더/입력은 [`crate::edithexview`].

/// HEX 편집 상한(전체 바이트를 메모리에 올린다). 초과 파일은 대용량 뷰어로 둔다.
pub(crate) const HEX_CAP: u64 = 16_000_000;

/// 한 줄에 표시할 바이트 수(고정 폭 레이아웃 기준).
pub(crate) const COLS: usize = 16;

/// 이진 편집 버퍼. 덮어쓰기/삽입 두 모드, 블록 선택·복사/붙여넣기·삽입/삭제 지원.
pub(crate) struct HexBuf {
    pub bytes: Vec<u8>,
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
    /// 실행 취소/다시 실행 스냅샷 스택((바이트, 커서)). 누적 크기는 MAX_UNDO_BYTES로 제한.
    pub undo: Vec<(Vec<u8>, usize)>,
    pub redo: Vec<(Vec<u8>, usize)>,
}

/// undo 히스토리 누적 바이트 상한(초과 시 오래된 스냅샷부터 버림).
pub(crate) const MAX_UNDO_BYTES: usize = 64_000_000;

impl HexBuf {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes, cursor: 0, anchor: None, low_nibble: false, ascii: false,
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
            self.cursor = off.min(self.bytes.len().saturating_sub(1));
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

    /// 파일 전체를 읽어 HEX 버퍼로 연다(상한 초과면 None).
    pub fn open(path: &std::path::Path) -> Option<Self> {
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
        if len > HEX_CAP {
            return None;
        }
        std::fs::read(path).ok().map(Self::from_bytes)
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// 표시 줄 수(마지막 부분 줄 포함).
    pub fn rows(&self) -> usize {
        self.bytes.len().div_ceil(COLS).max(1)
    }

    /// 커서를 delta만큼 이동(경계 클램프). select면 선택 확장. 이동하면 니블 상태 초기화.
    pub fn move_by(&mut self, delta: isize, select: bool) {
        self.update_anchor(select);
        let last = self.bytes.len().saturating_sub(1);
        self.cursor = (self.cursor as isize + delta).clamp(0, last as isize) as usize;
        self.low_nibble = false;
    }

    /// 줄 시작/끝으로(Home/End). select면 선택 확장.
    pub fn line_edge(&mut self, end: bool, select: bool) {
        self.update_anchor(select);
        let start = (self.cursor / COLS) * COLS;
        let stop = (start + COLS - 1).min(self.bytes.len().saturating_sub(1));
        self.cursor = if end { stop } else { start };
        self.low_nibble = false;
    }

    /// 데이터 인스펙터: 커서 위치 바이트를 u8/u16/u32(리틀엔디언) 정수로 해석한다(바이트 부족 시 None).
    pub fn inspect(&self) -> (Option<u8>, Option<u16>, Option<u32>) {
        let at = self.cursor;
        let b = self.bytes.get(at).copied();
        let u16v = (self.bytes.len() >= at + 2).then(|| u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]]));
        let u32v = (self.bytes.len() >= at + 4)
            .then(|| u32::from_le_bytes([self.bytes[at], self.bytes[at + 1], self.bytes[at + 2], self.bytes[at + 3]]));
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
        let at = self.cursor;
        if let Some(arr) = self.bytes.get(at..at + 8).and_then(|s| <[u8; 8]>::try_from(s).ok()) {
            let u = u64::from_le_bytes(arr);
            detail.push_str(&format!("\nu64 {u}   i64 {}   f64 {}", u as i64, f64::from_le_bytes(arr)));
        }
        // 커서부터 이어지는 인쇄 가능한 ASCII 문자열(바이너리 속 문자열 식별).
        let s = ascii_run(&self.bytes, at, 24);
        if !s.is_empty() {
            detail.push_str(&format!("\nstr \"{s}\""));
        }
        Some((brief, detail))
    }

    /// 전체 선택.
    pub fn select_all(&mut self) {
        if !self.bytes.is_empty() {
            self.anchor = Some(0);
            self.cursor = self.bytes.len() - 1;
        }
    }
}

impl crate::app::NabiApp {
    /// 텍스트↔HEX 편집 모드를 전환한다. HEX→텍스트는 현재 바이트를 인코딩으로 디코드,
    /// 텍스트→HEX는 현재 텍스트를 바이트로 적재한다(미저장 변경 유지).
    pub(crate) fn toggle_editor_hex(&mut self, pane: nabi_types::PaneId) {
        let Some(d) = self.editors.get_mut(&pane) else { return };
        if let Some(h) = d.hex.take() {
            let (text, encoding, eol) = crate::editload::decode(&h.bytes);
            d.text = text;
            d.encoding = encoding;
            d.eol = eol;
            d.dirty = h.dirty || d.dirty;
        } else if d.edit.is_none() && d.big.is_none() {
            // 일반 텍스트 버퍼만 HEX로 전환(대용량 rope/뷰어는 제외).
            let mut hb = HexBuf::from_bytes(std::mem::take(&mut d.text).into_bytes());
            hb.dirty = d.dirty;
            d.hex = Some(hb);
            d.encoding = "binary".into();
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
pub(crate) fn peek_is_binary(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut buf = [0u8; 8192];
    match f.read(&mut buf) {
        Ok(n) => is_binary(&buf[..n]),
        Err(_) => false,
    }
}

/// 앞부분 표본으로 이진 파일 여부를 추정한다(NUL 또는 제어문자 과다).
pub(crate) fn is_binary(bytes: &[u8]) -> bool {
    let n = bytes.len().min(8192);
    if n == 0 {
        return false;
    }
    let sample = &bytes[..n];
    if sample.contains(&0) {
        return true; // NUL 바이트 → 이진.
    }
    // 탭(09)·개행(0A)·캐리지(0D) 외 제어문자 비율이 높으면 이진.
    let ctrl = sample.iter().filter(|&&b| (b < 0x20 && !matches!(b, 0x09 | 0x0A | 0x0D)) || b == 0x7F).count();
    ctrl * 100 / n > 30
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
        assert_eq!(h.bytes[0], 0xAB);
        assert_eq!(h.cursor, 1);
        h.input_ascii(b'Z');
        assert_eq!(h.bytes[1], b'Z');
        assert!(h.dirty);
    }
}
