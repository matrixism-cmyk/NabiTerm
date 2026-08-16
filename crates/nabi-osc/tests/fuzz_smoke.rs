//! OSC 스캐너 퍼즈 스모크(T4-3) — 임의 바이트·기형 OSC가 스캐너를 죽이지 못하는지.
//! nabi-vt fuzz_smoke와 같은 계약: 원격 입력 표면은 절대 패닉 금지.

use nabi_osc::OscScanner;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 56) as u8
    }
}

#[test]
fn random_bytes_never_panic() {
    for seed in 1..=8u64 {
        let mut sc = OscScanner::default();
        let mut r = Rng(seed);
        for _ in 0..300 {
            let chunk: Vec<u8> = (0..256).map(|_| r.byte()).collect();
            let _ = sc.feed(&chunk);
        }
    }
}

/// 기형 OSC 편향: 프리픽스는 정상, 본문/종결이 임의(잘림·과대 번호·중첩 ESC).
#[test]
fn malformed_osc_never_panics() {
    let heads: &[&[u8]] = &[b"\x1b]0;", b"\x1b]7;", b"\x1b]52;", b"\x1b]133;", b"\x1b]9;", b"\x1b]777;", b"\x1b]99999;"];
    for seed in 1..=8u64 {
        let mut sc = OscScanner::default();
        let mut r = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15));
        for _ in 0..400 {
            let mut chunk = Vec::new();
            chunk.extend_from_slice(heads[(r.next() % heads.len() as u64) as usize]);
            for _ in 0..(r.next() % 64) {
                chunk.push(r.byte());
            }
            // 절반은 종결(BEL 또는 ST), 절반은 잘린 채 다음 조각으로.
            if r.next().is_multiple_of(2) {
                chunk.extend_from_slice(if r.next().is_multiple_of(2) { b"\x07" } else { b"\x1b\\" });
            }
            let _ = sc.feed(&chunk);
        }
    }
}
