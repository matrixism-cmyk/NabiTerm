//! iTerm(OSC 1337 File=) / Kitty(APC G) 이미지 페이로드 → RGBA 디코드.
//!
//! 표준 이미지 포맷(PNG/JPEG/GIF)은 `image` 크레이트로, Kitty raw(RGB/RGBA)는 직접 변환.

use crate::base64::base64_decode;
use crate::DecodedImage;

const MAX_PIXELS: u64 = 4000 * 4000;

/// 표준 이미지 바이트(PNG/JPEG/GIF 등)를 RGBA로 디코드한다.
pub fn decode_image_bytes(bytes: &[u8]) -> Option<DecodedImage> {
    let dyn_img = image::load_from_memory(bytes).ok()?;
    let rgba = dyn_img.to_rgba8();
    let (width, height) = rgba.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }
    Some(DecodedImage { width, height, rgba: rgba.into_raw() })
}

/// iTerm2 OSC 1337 본문(`1337;File=<args>:<base64>`)을 이미지로 디코드한다.
pub fn decode_iterm_osc(body: &[u8]) -> Option<DecodedImage> {
    let s = body.strip_prefix(b"1337;File=")?;
    let colon = s.iter().position(|&b| b == b':')?;
    let b64 = &s[colon + 1..]; // args(콜론 앞)는 무시 — 실제 픽셀 크기를 디코드로 얻는다.
    let raw = base64_decode(b64)?;
    decode_image_bytes(&raw)
}

/// Kitty 포맷별 조립: f=100 PNG, f=32 RGBA, f=24 RGB(→RGBA 확장).
pub fn kitty_assemble(fmt: u32, w: u32, h: u32, raw: &[u8]) -> Option<DecodedImage> {
    match fmt {
        100 => decode_image_bytes(raw),
        32 => {
            let need = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
            (raw.len() >= need && need > 0)
                .then(|| DecodedImage { width: w, height: h, rgba: raw[..need].to_vec() })
        }
        24 => {
            let px = (w as usize).checked_mul(h as usize)?;
            (raw.len() >= px * 3 && px > 0).then(|| {
                let mut rgba = Vec::with_capacity(px * 4);
                for c in raw[..px * 3].chunks_exact(3) {
                    rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
                }
                DecodedImage { width: w, height: h, rgba }
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_rgb_expands_to_rgba() {
        // 2x1 RGB(빨강,초록) → RGBA.
        let img = kitty_assemble(24, 2, 1, &[255, 0, 0, 0, 255, 0]).unwrap();
        assert_eq!(img.rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn kitty_rgba_passthrough() {
        let img = kitty_assemble(32, 1, 1, &[1, 2, 3, 4]).unwrap();
        assert_eq!((img.width, img.height, img.rgba), (1, 1, vec![1, 2, 3, 4]));
    }

    #[test]
    fn iterm_needs_file_prefix() {
        assert!(decode_iterm_osc(b"1337;SetMark").is_none());
    }

    #[test]
    fn decodes_png_roundtrip() {
        use image::{ImageFormat, Rgba, RgbaImage};
        let mut im = RgbaImage::new(3, 2);
        im.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(im).write_to(&mut buf, ImageFormat::Png).unwrap();
        let dec = decode_image_bytes(buf.get_ref()).unwrap();
        assert_eq!((dec.width, dec.height), (3, 2));
        assert_eq!(&dec.rgba[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn iterm_osc_decodes_embedded_png() {
        use image::{ImageFormat, RgbaImage};
        let im = RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(im).write_to(&mut buf, ImageFormat::Png).unwrap();
        // OSC 1337 File=...:base64(png) 형태로 조립해 디코드.
        let b64 = b64encode(buf.get_ref());
        let mut body = b"1337;File=inline=1:".to_vec();
        body.extend_from_slice(b64.as_bytes());
        let dec = decode_iterm_osc(&body).unwrap();
        assert_eq!((dec.width, dec.height), (2, 2));
    }

    // 테스트 전용 base64 인코더.
    fn b64encode(data: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in data.chunks(3) {
            let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for k in 0..4 {
                if k <= c.len() {
                    s.push(A[((n >> (18 - 6 * k)) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }
}
