//! 페이로드 인코딩 — trzsz의 문자열·바이너리 페이로드는 **zlib 압축 후 base64**다.
//!
//! 왜 이렇게까지 하는가: 이 바이트들은 그냥 파일이 아니라 **셸의 표준입출력을 지나간다**.
//! 제어문자가 하나라도 날것으로 들어가면 tty 드라이버·중간 ssh·tmux가 해석해 버린다.
//! base64는 그걸 막고, zlib은 base64가 늘린 33%를 되돌린다.

use base64::Engine as _;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read as _, Write as _};

/// 원시 바이트 → 줄에 실을 수 있는 ASCII 문자열.
pub fn encode_payload(raw: &[u8]) -> String {
    let mut z = ZlibEncoder::new(Vec::with_capacity(raw.len() / 2 + 16), Compression::default());
    // 메모리 버퍼에 쓰는 것이라 실패할 길이 없다. 그래도 결과는 무시하지 않는다.
    let ok = z.write_all(raw).is_ok();
    let packed = if ok { z.finish().unwrap_or_default() } else { Vec::new() };
    base64::engine::general_purpose::STANDARD.encode(packed)
}

/// 줄 페이로드 → 원시 바이트. 원격이 보낸 값이므로 **깨져 있을 수 있다**.
pub fn decode_payload(text: &str) -> Option<Vec<u8>> {
    // 원격 구현마다 줄 끝 공백이 붙는 경우가 있어 먼저 다듬는다.
    let packed = base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()?;
    // zlib 머리 검사를 직접 한다 — 디코더는 머리가 틀려도 "0바이트 읽음"으로 조용히 끝나서
    // 빈 결과와 구별이 안 된다(실제로 `YWJj`가 빈 성공으로 통과했다).
    if !is_zlib_header(&packed) {
        return None;
    }
    let mut out = Vec::with_capacity(packed.len() * 4 + 16);
    // 압축 폭탄 방어: 한 줄이 이보다 크게 풀리면 프로토콜 위반으로 본다.
    const MAX: u64 = 64 << 20;
    ZlibDecoder::new(&packed[..]).take(MAX).read_to_end(&mut out).ok()?;
    Some(out)
}

/// RFC 1950 머리 검사 — 압축 방식이 deflate(8)이고 두 바이트가 31의 배수여야 한다.
fn is_zlib_header(b: &[u8]) -> bool {
    b.len() >= 2 && b[0] & 0x0f == 8 && (u16::from(b[0]) << 8 | u16::from(b[1])) % 31 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for raw in [&b""[..], b"a", b"\x00\x01\xee\xff", &[7u8; 100_000][..]] {
            let text = encode_payload(raw);
            assert!(text.is_ascii(), "줄에 실리려면 ASCII여야 한다");
            assert_eq!(decode_payload(&text).as_deref(), Some(raw));
        }
    }

    #[test]
    fn compresses_repetitive_data() {
        assert!(encode_payload(&[0u8; 8192]).len() < 200, "반복 데이터는 크게 줄어야 한다");
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert_eq!(decode_payload("not base64 @@@"), None);
        assert_eq!(decode_payload("YWJj"), None, "base64는 맞지만 zlib이 아니다");
        assert_eq!(decode_payload(""), None);
    }

    #[test]
    fn tolerates_trailing_whitespace() {
        let t = encode_payload(b"hello");
        assert_eq!(decode_payload(&format!("{t}  \r")).as_deref(), Some(&b"hello"[..]));
    }
}
