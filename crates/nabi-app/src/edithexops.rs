//! nabiPad HEX 비트 연산(HxD/010 벤치마킹) — 선택(없으면 전체) 바이트에 적용. undo 지원.

use crate::edithex::HexBuf;

impl HexBuf {
    /// 연산 대상 범위 [lo, hi)(선택 있으면 그 범위, 없으면 전체).
    fn op_range(&self) -> (usize, usize) {
        self.selection().map(|(a, b)| (a, b.min(self.bytes.len()))).unwrap_or((0, self.bytes.len()))
    }

    /// 대상 바이트마다 f를 적용(길이 불변). 바뀐 구간만 undo에 기록된다.
    fn map_bytes(&mut self, f: impl Fn(u8) -> u8) {
        let (lo, hi) = self.op_range();
        if lo >= hi {
            return;
        }
        let new: Vec<u8> = self.bytes[lo..hi].iter().map(|b| f(*b)).collect();
        self.splice(lo, hi - lo, &new, false);
    }

    /// 대상 구간을 통째로 다시 만들어 갈아 끼운다(순서 바꾸기 계열 공용).
    fn rebuild(&mut self, min_len: usize, f: impl Fn(&mut [u8])) {
        let (lo, hi) = self.op_range();
        if hi.saturating_sub(lo) < min_len {
            return;
        }
        let mut new = self.bytes[lo..hi].to_vec();
        f(&mut new);
        self.splice(lo, hi - lo, &new, false);
    }

    /// 비트 반전(NOT).
    pub(crate) fn invert(&mut self) {
        self.map_bytes(|b| !b);
    }

    /// 상·하위 니블 교환(0x12→0x21).
    pub(crate) fn swap_nibbles(&mut self) {
        self.map_bytes(|b| b.rotate_left(4));
    }

    /// 각 바이트 +1(래핑).
    pub(crate) fn increment(&mut self) {
        self.map_bytes(|b| b.wrapping_add(1));
    }

    /// 각 바이트 -1(래핑).
    pub(crate) fn decrement(&mut self) {
        self.map_bytes(|b| b.wrapping_sub(1));
    }

    /// 각 바이트를 왼쪽으로 1비트 시프트(`<<1`).
    pub(crate) fn shift_left(&mut self) {
        self.map_bytes(|b| b << 1);
    }

    /// 각 바이트를 오른쪽으로 1비트 시프트(`>>1`).
    pub(crate) fn shift_right(&mut self) {
        self.map_bytes(|b| b >> 1);
    }

    /// 각 바이트를 왼쪽으로 1비트 회전(최상위→최하위).
    pub(crate) fn rotate_left(&mut self) {
        self.map_bytes(|b| b.rotate_left(1));
    }

    /// 각 바이트를 오른쪽으로 1비트 회전.
    pub(crate) fn rotate_right(&mut self) {
        self.map_bytes(|b| b.rotate_right(1));
    }

    /// 각 바이트의 비트 순서를 뒤집는다(0b1000_0001→0b1000_0001, 0x01→0x80). 시리얼/LSB 우선 데이터용.
    pub(crate) fn reverse_bits(&mut self) {
        self.map_bytes(|b| b.reverse_bits());
    }

    /// 16비트 워드 단위로 바이트를 맞바꾼다(리틀↔빅 엔디언, 2바이트씩). 홀수 끝 바이트는 유지.
    pub(crate) fn swap_bytes16(&mut self) {
        self.rebuild(2, |s| {
            for c in s.chunks_exact_mut(2) {
                c.swap(0, 1);
            }
        });
    }

    /// 32비트 워드 단위로 바이트 순서를 뒤집는다(4바이트씩). 끝 1~3바이트는 유지.
    pub(crate) fn swap_bytes32(&mut self) {
        self.rebuild(4, |s| {
            for c in s.chunks_exact_mut(4) {
                c.reverse();
            }
        });
    }

    /// 대상 범위의 바이트 순서를 뒤집는다(엔디언 전환 등).
    pub(crate) fn reverse_bytes(&mut self) {
        self.rebuild(1, |s| s.reverse());
    }
}

#[cfg(test)]
mod tests {
    use crate::edithex::HexBuf;

    #[test]
    fn invert_and_nibble() {
        let mut h = HexBuf::from_bytes(vec![0x0F, 0xF0]);
        h.invert();
        assert_eq!(h.bytes, vec![0xF0, 0x0F]);
        let mut h = HexBuf::from_bytes(vec![0x12, 0x34]);
        h.swap_nibbles();
        assert_eq!(h.bytes, vec![0x21, 0x43]);
    }

    #[test]
    fn inc_dec_reverse() {
        let mut h = HexBuf::from_bytes(vec![0xFF, 0x00]);
        h.increment();
        assert_eq!(h.bytes, vec![0x00, 0x01]);
        h.decrement();
        assert_eq!(h.bytes, vec![0xFF, 0x00]);
        let mut h = HexBuf::from_bytes(vec![1, 2, 3]);
        h.reverse_bytes();
        assert_eq!(h.bytes, vec![3, 2, 1]);
        let mut h = HexBuf::from_bytes(vec![0x01, 0x80]);
        h.shift_left();
        assert_eq!(h.bytes, vec![0x02, 0x00]);
        h.shift_right();
        assert_eq!(h.bytes, vec![0x01, 0x00]);
        let mut h = HexBuf::from_bytes(vec![0x81]);
        h.rotate_left();
        assert_eq!(h.bytes, vec![0x03]); // 1000_0001 → 0000_0011.
        h.rotate_right();
        assert_eq!(h.bytes, vec![0x81]);
        let mut h = HexBuf::from_bytes(vec![0x01, 0x02]);
        h.reverse_bits();
        assert_eq!(h.bytes, vec![0x80, 0x40]); // 비트 역순.
        let mut h = HexBuf::from_bytes(vec![0x01, 0x02, 0x03, 0x04, 0x05]);
        h.swap_bytes16();
        assert_eq!(h.bytes, vec![0x02, 0x01, 0x04, 0x03, 0x05]); // 2바이트씩 스왑, 끝 1바이트 유지.
        let mut h = HexBuf::from_bytes(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        h.swap_bytes32();
        assert_eq!(h.bytes, vec![4, 3, 2, 1, 8, 7, 6, 5, 9]); // 4바이트씩 역순, 끝 1바이트 유지.
    }

    #[test]
    fn selection_scoped() {
        let mut h = HexBuf::from_bytes(vec![0x00, 0x00, 0x00]);
        h.anchor = Some(1);
        h.cursor = 1; // 선택 [1,2) = 두 번째 바이트만.
        h.invert();
        assert_eq!(h.bytes, vec![0x00, 0xFF, 0x00]);
    }
}
