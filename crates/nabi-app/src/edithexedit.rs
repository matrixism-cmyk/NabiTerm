//! HEX 편집 연산 — 블록 선택 삭제·삽입/덮어쓰기 입력·복사/붙여넣기. edithex.rs 모델 위에서 동작.
//! 실제 HEX 에디터처럼 중간 삽입/삭제로 바이트 수가 변할 수 있다(삽입 모드).

use crate::edithex::{HexBuf, MAX_UNDO_BYTES};

impl HexBuf {
    /// 변경 직전 현재 상태를 undo 스택에 저장하고 redo를 비운다(누적 크기 상한 적용).
    pub(crate) fn push_undo(&mut self) {
        self.undo.push((self.bytes.clone(), self.cursor));
        self.redo.clear();
        let mut total: usize = self.undo.iter().map(|(b, _)| b.len()).sum();
        while self.undo.len() > 1 && total > MAX_UNDO_BYTES {
            total -= self.undo.remove(0).0.len();
        }
    }

    /// 실행 취소.
    pub fn undo(&mut self) {
        if let Some((bytes, cur)) = self.undo.pop() {
            self.redo.push((std::mem::replace(&mut self.bytes, bytes), self.cursor));
            self.cursor = cur.min(self.bytes.len().saturating_sub(1));
            self.low_nibble = false;
            self.anchor = None;
            self.dirty = true;
        }
    }

    /// 다시 실행.
    pub fn redo(&mut self) {
        if let Some((bytes, cur)) = self.redo.pop() {
            self.undo.push((std::mem::replace(&mut self.bytes, bytes), self.cursor));
            self.cursor = cur.min(self.bytes.len().saturating_sub(1));
            self.low_nibble = false;
            self.anchor = None;
            self.dirty = true;
        }
    }

    /// 선택이 있으면 그 범위를 지우고 커서를 시작으로 옮긴다(입력/붙여넣기 전 호출). 지웠으면 true.
    fn drop_selection(&mut self) -> bool {
        if let Some((lo, hi)) = self.selection() {
            let hi = hi.min(self.bytes.len());
            self.bytes.drain(lo..hi);
            self.cursor = lo.min(self.bytes.len().saturating_sub(1));
            self.anchor = None;
            self.low_nibble = false;
            self.dirty = true;
            return true;
        }
        false
    }

    /// HEX 니블 입력(d=0..=15). 삽입 모드(또는 끝)면 새 바이트를 끼우고, 아니면 덮어쓴다.
    pub fn input_nibble(&mut self, d: u8) {
        if !self.low_nibble {
            self.push_undo(); // 새 바이트 시작 시에만 스냅샷(니블 2회를 한 단위로).
        }
        self.drop_selection();
        if self.low_nibble {
            if let Some(b) = self.bytes.get_mut(self.cursor) {
                *b = (*b & 0xF0) | (d & 0x0F);
            }
            self.low_nibble = false;
            self.dirty = true;
            self.move_by(1, false);
        } else {
            let at = self.cursor.min(self.bytes.len());
            if self.insert_mode || at >= self.bytes.len() {
                self.bytes.insert(at, d << 4);
                self.cursor = at;
            } else {
                self.bytes[at] = (self.bytes[at] & 0x0F) | (d << 4);
            }
            self.low_nibble = true;
            self.dirty = true;
        }
    }

    /// ASCII 칼럼 입력(삽입/덮어쓰기 후 진행).
    pub fn input_ascii(&mut self, c: u8) {
        self.push_undo();
        self.drop_selection();
        let at = self.cursor.min(self.bytes.len());
        if self.insert_mode || at >= self.bytes.len() {
            self.bytes.insert(at, c);
        } else {
            self.bytes[at] = c;
        }
        self.dirty = true;
        self.move_by(1, false);
    }

    /// 바이트열을 커서 위치에 삽입(붙여넣기). 선택이 있으면 대체.
    pub fn insert_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.push_undo();
        self.drop_selection();
        let at = self.cursor.min(self.bytes.len());
        self.bytes.splice(at..at, data.iter().copied());
        self.cursor = (at + data.len()).min(self.bytes.len().saturating_sub(1));
        self.low_nibble = false;
        self.dirty = true;
    }

    /// Delete: 선택 있으면 선택 삭제, 없으면 커서 바이트 삭제.
    pub fn delete_forward(&mut self) {
        if self.selection().is_none() && self.cursor >= self.bytes.len() {
            return; // 지울 게 없으면 undo 기록도 남기지 않음.
        }
        self.push_undo();
        if self.drop_selection() || self.cursor >= self.bytes.len() {
            return;
        }
        self.bytes.remove(self.cursor);
        self.cursor = self.cursor.min(self.bytes.len().saturating_sub(1));
        self.low_nibble = false;
        self.dirty = true;
    }

    /// Backspace: 선택 있으면 선택 삭제, 없으면 앞 바이트 삭제.
    pub fn backspace(&mut self) {
        if self.selection().is_none() && self.cursor == 0 {
            return;
        }
        self.push_undo();
        if self.drop_selection() || self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.bytes.remove(self.cursor);
        self.low_nibble = false;
        self.dirty = true;
    }

    /// 선택 영역을 주어진 바이트 값으로 채운다(길이 불변).
    pub fn fill_selection(&mut self, byte: u8) {
        if self.selection().is_none() {
            return;
        }
        self.push_undo();
        if let Some((lo, hi)) = self.selection() {
            let hi = hi.min(self.bytes.len());
            for b in &mut self.bytes[lo..hi] {
                *b = byte;
            }
            self.dirty = true;
        }
    }

    /// 선택 바이트(없으면 빈 벡터).
    pub fn selected_bytes(&self) -> Vec<u8> {
        match self.selection() {
            Some((lo, hi)) => self.bytes[lo..hi.min(self.bytes.len())].to_vec(),
            None => Vec::new(),
        }
    }
}

/// 바이트열 → "DE AD BE EF" 대문자 HEX 문자열(클립보드 복사용).
pub(crate) fn to_hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// 바이트열 → C 배열 리터럴 본문 "0xDE, 0xAD, 0xBE"(코드 임베드용).
pub(crate) fn to_c_array(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("0x{b:02X}")).collect::<Vec<_>>().join(", ")
}

/// 문자열에서 HEX 숫자만 골라 바이트열로(붙여넣기). 공백 등은 무시, 홀수 니블은 버린다.
pub(crate) fn parse_hex(s: &str) -> Vec<u8> {
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
        assert_eq!(h.bytes, vec![1, 4]);
        h.cursor = 1;
        h.insert_bytes(&[9, 9]); // 1 [9 9] 4
        assert_eq!(h.bytes, vec![1, 9, 9, 4]);
    }

    #[test]
    fn insert_mode_nibble_grows() {
        let mut h = HexBuf::from_bytes(vec![0xAA]);
        h.insert_mode = true;
        h.cursor = 0;
        h.input_nibble(0xB);
        h.input_nibble(0xC); // 0xBC 삽입
        assert_eq!(h.bytes, vec![0xBC, 0xAA]);
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut h = HexBuf::from_bytes(vec![1, 2, 3]);
        h.input_ascii(9); // bytes[0]=9
        assert_eq!(h.bytes[0], 9);
        h.undo();
        assert_eq!(h.bytes, vec![1, 2, 3]);
        h.redo();
        assert_eq!(h.bytes[0], 9);
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
