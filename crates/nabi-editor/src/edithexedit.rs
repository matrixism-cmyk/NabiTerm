//! HEX 편집 연산 — 블록 선택 삭제·삽입/덮어쓰기 입력·복사/붙여넣기. edithex.rs 모델 위에서 동작.
//! 실제 HEX 에디터처럼 중간 삽입/삭제로 바이트 수가 변할 수 있다(삽입 모드).

use crate::edithex::{HexBuf, HexDelta, MAX_UNDO_BYTES};

impl HexBuf {
    /// `bytes[at..at+del]`을 `new`로 바꾸고 **그 구간만** undo에 기록한다.
    ///
    /// `group=true`면 직전 기록의 결과만 갱신한다(HEX 니블 두 번을 한 취소 단위로).
    pub fn splice(&mut self, at: usize, del: usize, new: &[u8], group: bool) {
        self.splice_ex(at, del, new, group, false);
    }

    /// `splice`와 같지만 이 기록을 **앞 기록과 한 취소 단위로 묶는다**(모두 바꾸기 등).
    pub fn splice_chained(&mut self, at: usize, del: usize, new: &[u8], chain: bool) {
        self.splice_ex(at, del, new, false, chain);
    }

    fn splice_ex(&mut self, at: usize, del: usize, new: &[u8], group: bool, cont: bool) {
        let at = at.min(self.data.len());
        let end = (at + del).min(self.data.len());
        let old = self.data.read(at, end - at);
        if old == *new {
            return; // 실제 변화가 없으면 기록도 남기지 않는다.
        }
        self.data.splice(at, end - at, new);
        self.dirty = true;
        if group {
            if let Some(p) = self.undo.last_mut() {
                if p.at == at && p.new.len() == old.len() {
                    p.new = new.to_vec();
                    return;
                }
            }
        }
        self.undo.push(HexDelta { at, old, new: new.to_vec(), cur: self.cursor, cont });
        self.redo.clear();
        let mut total: usize = self.undo.iter().map(|d| d.old.len() + d.new.len()).sum();
        while self.undo.len() > 1 && total > MAX_UNDO_BYTES {
            let d = self.undo.remove(0);
            total -= d.old.len() + d.new.len();
        }
    }

    /// 실행 취소. 묶인 기록(`cont`)은 **한 번에** 전부 되돌린다.
    pub fn undo(&mut self) {
        if self.undo.is_empty() {
            return;
        }
        while let Some(d) = self.undo.pop() {
            let end = (d.at + d.new.len()).min(self.data.len());
            self.data.splice(d.at, end - d.at, &d.old);
            self.cursor = d.cur.min(self.data.len().saturating_sub(1));
            let more = d.cont; // 이 기록이 앞 기록과 한 단위면 이어서 되돌린다.
            self.redo.push(d);
            if !more {
                break;
            }
        }
        self.after_history();
    }

    /// 다시 실행. 취소와 대칭으로, 묶인 기록은 한 번에 전부 다시 적용한다.
    pub fn redo(&mut self) {
        if self.redo.is_empty() {
            return;
        }
        while let Some(d) = self.redo.pop() {
            let end = (d.at + d.old.len()).min(self.data.len());
            self.data.splice(d.at, end - d.at, &d.new);
            self.cursor = (d.at + d.new.len()).saturating_sub(1).min(self.data.len().saturating_sub(1));
            self.undo.push(d);
            // 다음 기록이 같은 묶음이면 이어서 적용한다.
            if !self.redo.last().is_some_and(|n| n.cont) {
                break;
            }
        }
        self.after_history();
    }

    fn after_history(&mut self) {
        self.low_nibble = false;
        self.anchor = None;
        self.dirty = true;
    }

    /// 입력/붙여넣기가 덮어쓸 범위 — 선택이 있으면 그 범위, 없으면 커서 자리의 빈 범위.
    fn target(&self) -> (usize, usize) {
        match self.selection() {
            Some((lo, hi)) => (lo, hi.min(self.data.len())),
            None => {
                let c = self.cursor.min(self.data.len());
                (c, c)
            }
        }
    }

    /// 입력 후 커서·선택 정리(공통).
    fn after_input(&mut self, at: usize) {
        self.cursor = at.min(self.data.len().saturating_sub(1));
        self.anchor = None;
        self.low_nibble = false;
    }

    /// HEX 니블 입력(d=0..=15). 삽입 모드(또는 끝)면 새 바이트를 끼우고, 아니면 덮어쓴다.
    /// 선택이 있으면 그 범위를 입력한 바이트로 대체한다(HxD와 같은 동작).
    pub fn input_nibble(&mut self, d: u8) {
        if self.low_nibble {
            // 같은 바이트의 하위 니블 — 상위 니블 기록에 합쳐 한 번에 취소되게 한다.
            let at = self.cursor.min(self.data.len().saturating_sub(1));
            if let Some(b) = self.data.get(at) {
                self.splice(at, 1, &[(b & 0xF0) | (d & 0x0F)], true);
            }
            self.low_nibble = false;
            self.move_by(1, false);
            return;
        }
        let (lo, hi) = self.target();
        let overwrite = lo == hi && !self.insert_mode && lo < self.data.len();
        let del = if lo != hi { hi - lo } else { usize::from(overwrite) };
        let keep = if overwrite { self.data.get(lo).unwrap_or(0) & 0x0F } else { 0 }; // 덮어쓰기는 하위 니블 보존.
        self.splice(lo, del, &[(d << 4) | keep], false);
        self.after_input(lo);
        self.low_nibble = true;
    }

    /// ASCII 칼럼 입력(삽입/덮어쓰기 후 진행).
    pub fn input_ascii(&mut self, c: u8) {
        let (lo, hi) = self.target();
        let overwrite = lo == hi && !self.insert_mode && lo < self.data.len();
        let del = if lo != hi { hi - lo } else { usize::from(overwrite) };
        self.splice(lo, del, &[c], false);
        self.after_input(lo);
        self.move_by(1, false);
    }

    /// 바이트열을 커서 위치에 삽입(붙여넣기). 선택이 있으면 대체.
    pub fn insert_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let (lo, hi) = self.target();
        self.splice(lo, hi - lo, data, false);
        self.after_input(lo + data.len());
    }

    /// Delete: 선택 있으면 선택 삭제, 없으면 커서 바이트 삭제.
    pub fn delete_forward(&mut self) {
        let (lo, hi) = self.target();
        let del = if lo != hi {
            hi - lo
        } else if lo < self.data.len() {
            1
        } else {
            return; // 지울 게 없으면 기록도 남기지 않는다.
        };
        self.splice(lo, del, &[], false);
        self.after_input(lo);
    }

    /// Backspace: 선택 있으면 선택 삭제, 없으면 앞 바이트 삭제.
    pub fn backspace(&mut self) {
        let (lo, hi) = self.target();
        let (at, del) = if lo != hi {
            (lo, hi - lo)
        } else if lo > 0 {
            (lo - 1, 1)
        } else {
            return;
        };
        self.splice(at, del, &[], false);
        self.after_input(at);
    }

    /// 선택 영역을 주어진 바이트 값으로 채운다(길이 불변).
    pub fn fill_selection(&mut self, byte: u8) {
        let Some((lo, hi)) = self.selection() else { return };
        let hi = hi.min(self.data.len());
        if lo < hi {
            self.splice(lo, hi - lo, &vec![byte; hi - lo], false);
        }
    }

    /// 선택 바이트(없으면 빈 벡터).
    pub fn selected_bytes(&self) -> Vec<u8> {
        match self.selection() {
            Some((lo, hi)) => self.range(lo, hi.min(self.data.len())),
            None => Vec::new(),
        }
    }
}

/// 바이트열 → "DE AD BE EF" 대문자 HEX 문자열(클립보드 복사용).
pub fn to_hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// 바이트열 → C 배열 리터럴 본문 "0xDE, 0xAD, 0xBE"(코드 임베드용).
pub fn to_c_array(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("0x{b:02X}")).collect::<Vec<_>>().join(", ")
}

/// 문자열에서 HEX 숫자만 골라 바이트열로(붙여넣기). 공백 등은 무시, 홀수 니블은 버린다.
pub fn parse_hex(s: &str) -> Vec<u8> {
    let nibs: Vec<u8> = s.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8)).collect();
    nibs.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0] << 4) | c[1]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_delete_and_insert() {
        let mut h = HexBuf::from_bytes(vec![1, 2, 3, 4]);
        h.anchor = Some(1);
        h.cursor = 2; // 선택 [1,3) = {2,3}
        assert_eq!(h.selected_bytes(), vec![2, 3]);
        h.delete_forward(); // 선택 삭제 → [1,4]
        assert_eq!(h.bytes(), vec![1, 4]);
        h.cursor = 1;
        h.insert_bytes(&[9, 9]); // 1 [9 9] 4
        assert_eq!(h.bytes(), vec![1, 9, 9, 4]);
    }

    #[test]
    fn insert_mode_nibble_grows() {
        let mut h = HexBuf::from_bytes(vec![0xAA]);
        h.insert_mode = true;
        h.cursor = 0;
        h.input_nibble(0xB);
        h.input_nibble(0xC); // 0xBC 삽입
        assert_eq!(h.bytes(), vec![0xBC, 0xAA]);
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut h = HexBuf::from_bytes(vec![1, 2, 3]);
        h.input_ascii(9); // bytes[0]=9
        assert_eq!(h.at(0), Some(9));
        h.undo();
        assert_eq!(h.bytes(), vec![1, 2, 3]);
        h.redo();
        assert_eq!(h.at(0), Some(9));
    }

    #[test]
    fn undo_records_only_changed_region() {
        // 큰 버퍼에서 한 바이트를 고쳐도 기록은 그 한 바이트뿐이어야 한다(전체 복제 금지).
        let mut h = HexBuf::from_bytes(vec![0; 100_000]);
        h.cursor = 5_000;
        h.input_ascii(7);
        assert_eq!(h.undo.len(), 1);
        assert_eq!((h.undo[0].at, h.undo[0].old.len(), h.undo[0].new.len()), (5_000, 1, 1));
    }

    #[test]
    fn two_nibbles_undo_as_one_byte() {
        let mut h = HexBuf::from_bytes(vec![0x00, 0xFF]);
        h.cursor = 0;
        h.input_nibble(0xA);
        h.input_nibble(0xB); // 0xAB — 니블 두 번이 한 취소 단위.
        assert_eq!(h.bytes(), vec![0xAB, 0xFF]);
        assert_eq!(h.undo.len(), 1, "니블 2회는 기록 하나");
        h.undo();
        assert_eq!(h.bytes(), vec![0x00, 0xFF]);
        h.redo();
        assert_eq!(h.bytes(), vec![0xAB, 0xFF]);
    }

    #[test]
    fn length_changing_edits_undo_exactly() {
        let mut h = HexBuf::from_bytes(vec![1, 2, 3, 4]);
        h.cursor = 1;
        h.insert_bytes(&[9, 9]); // 길이 증가
        assert_eq!(h.bytes(), vec![1, 9, 9, 2, 3, 4]);
        h.undo();
        assert_eq!(h.bytes(), vec![1, 2, 3, 4], "삽입은 정확히 되돌아온다");
        h.redo();
        assert_eq!(h.bytes(), vec![1, 9, 9, 2, 3, 4]);
        h.undo();
        h.anchor = Some(1);
        h.cursor = 2;
        h.backspace(); // 선택 [1,3) 삭제 = 길이 감소
        assert_eq!(h.bytes(), vec![1, 4]);
        h.undo();
        assert_eq!(h.bytes(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn no_change_leaves_no_history() {
        // 같은 값을 다시 써도 취소 기록이 쌓이면, Ctrl+Z가 아무 일도 안 하는 것처럼 보인다.
        let mut h = HexBuf::from_bytes(vec![0x41]);
        h.cursor = 0;
        h.input_ascii(0x41);
        assert!(h.undo.is_empty());
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(to_hex_string(&[0xDE, 0xAD]), "DE AD");
        assert_eq!(parse_hex("DE AD be ef"), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn c_array_format() {
        assert_eq!(to_c_array(&[0xDE, 0xAD, 0x00]), "0xDE, 0xAD, 0x00");
        assert_eq!(to_c_array(&[]), "");
    }
}
