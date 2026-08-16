//! VT 파서 퍼즈 스모크(T4-3) — 원격이 보낼 수 있는 임의 바이트가 파서를 죽이지 못하는지.
//!
//! 렌더러 패닉 = UI 스레드 즉사(과거 v0.1.41 사고)라, 원격 입력 표면은 "절대 패닉 금지"가
//! 계약이다. coverage-guided 퍼저(cargo-fuzz)는 nightly+libFuzzer가 필요해 게이트에 못 넣으므로,
//! 여기서는 **결정적 시드**의 유사난수 스트림을 대량 주입하는 스모크를 상시 게이트로 돌린다.
//! (같은 시드는 항상 같은 스트림 — 실패 재현이 즉시 된다.)

use nabi_types::GridSize;
use nabi_vt::TermModel;

/// xorshift64* — 외부 의존 없이 결정적 바이트 스트림.
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

/// 완전 무작위 바이트(제어문자 포함) — 인코딩 깨짐·잘린 UTF-8·임의 C0/C1.
#[test]
fn random_bytes_never_panic() {
    for seed in 1..=8u64 {
        let mut m = TermModel::new(GridSize::new(80, 24), 200);
        let mut r = Rng(seed);
        for _ in 0..200 {
            let chunk: Vec<u8> = (0..512).map(|_| r.byte()).collect();
            m.process(&chunk);
            let _ = m.take_replies();
        }
        let _ = m.rows_cached(&nabi_vt::Theme::default()); // 렌더 경로도 최소 한 번.
    }
}

/// 이스케이프 시퀀스 편향 스트림 — CSI/OSC/DCS 프리픽스 뒤에 임의 파라미터·중간바이트.
#[test]
fn escape_soup_never_panics() {
    let prefixes: &[&[u8]] = &[b"\x1b[", b"\x1b]", b"\x1bP", b"\x1b_", b"\x1b^", b"\x1b[?", b"\x1b[>", b"\x1b[="];
    for seed in 1..=8u64 {
        let mut m = TermModel::new(GridSize::new(40, 10), 100);
        let mut r = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15));
        for _ in 0..400 {
            let mut chunk = Vec::with_capacity(64);
            chunk.extend_from_slice(prefixes[(r.next() % prefixes.len() as u64) as usize]);
            for _ in 0..(r.next() % 40) {
                // 파라미터·세미콜론·종결자 대역에 편향(파서 상태기계를 깊게 흔든다).
                let b = match r.next() % 5 {
                    0 => b'0' + (r.byte() % 10),
                    1 => b';',
                    2 => 0x40 + (r.byte() % 0x3f), // 종결자 대역.
                    3 => r.byte() % 0x20,          // C0 삽입.
                    _ => r.byte(),
                };
                chunk.push(b);
            }
            m.process(&chunk);
            let _ = m.take_replies();
        }
        m.process(b"\x1b[2J\x1b[H normal text \r\n");
        let _ = m.rows_cached(&nabi_vt::Theme::default());
    }
}

/// 리사이즈를 섞은 스트림 — 크기 변화 도중의 시퀀스가 인덱스를 어긋내지 않는지
/// (painter 인덱스 패닉 사고와 같은 클래스).
#[test]
fn resize_interleaved_never_panics() {
    let mut m = TermModel::new(GridSize::new(80, 24), 100);
    let mut r = Rng(0xDEADBEEF);
    for i in 0..300 {
        let chunk: Vec<u8> = (0..128).map(|_| r.byte()).collect();
        m.process(&chunk);
        if i % 7 == 0 {
            let cols = 2 + (r.next() % 200) as u16;
            let rows = 1 + (r.next() % 80) as u16;
            m.resize(GridSize::new(cols, rows));
        }
        let _ = m.rows_cached(&nabi_vt::Theme::default());
    }
}
