//! nabi-image — 터미널 인라인 이미지 디코더(현재 Sixel). 순수 Rust(외부 C 의존 없음).
//!
//! Sixel(DCS) 페이로드를 RGBA 비트맵으로 디코드한다. Kitty/iTerm은 후속.

mod base64;
mod raster;
mod sixel;
pub use base64::base64_decode;
pub use raster::{decode_image_bytes, decode_iterm_osc, kitty_assemble};
pub use sixel::decode_sixel;

/// 디코드된 RGBA 이미지(비설정 픽셀은 alpha=0 투명).
#[derive(Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// width*height*4 (R,G,B,A) 행우선.
    pub rgba: Vec<u8>,
}
