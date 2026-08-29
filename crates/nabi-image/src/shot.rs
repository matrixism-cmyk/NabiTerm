//! 화면에서 읽은 점들을 **PNG 파일로 만든다**(배치 AN).
//!
//! 윈도우에서 화면을 읽으면 점 하나가 파랑·초록·빨강·안 씀 차례로 들어온다(BGRA).
//! PNG 는 빨강·초록·파랑·투명도 차례를 쓴다(RGBA). 그래서 자리를 바꿔 줘야 한다.
//!
//! 이 파일은 화면을 만지지 않는다 — 받은 바이트만 다룬다. 그래서 시험할 수 있다.

/// 화면에서 읽은 BGRA 를 PNG 가 쓰는 RGBA 로 바꾼다.
///
/// 윈도우가 주는 그림은 **투명도 자리가 비어 있다**(늘 0). 그대로 두면 PNG 가 전부
/// 투명한 그림이 되어 아무것도 안 보인다. 그래서 불투명으로 채운다.
pub fn bgra_to_rgba(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

/// PNG 로 저장한다.
pub fn save_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if w == 0 || h == 0 {
        return Err("크기가 0이라 저장할 것이 없다".into());
    }
    let need = (w as usize) * (h as usize) * 4;
    if rgba.len() < need {
        return Err(format!("점이 모자란다: {need}개 필요한데 {}개다", rgba.len()));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("폴더를 만들지 못했다: {e}"))?;
    }
    image::RgbaImage::from_raw(w, h, rgba[..need].to_vec())
        .ok_or_else(|| "그림을 만들지 못했다".to_string())?
        .save(path)
        .map_err(|e| format!("저장하지 못했다: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{bgra_to_rgba, save_png};

    #[test]
    fn blue_and_red_swap_places() {
        // 파랑 255, 초록 128, 빨강 64 → 빨강 64, 초록 128, 파랑 255.
        let mut b = [255u8, 128, 64, 0];
        bgra_to_rgba(&mut b);
        assert_eq!(b, [64, 128, 255, 255]);
    }

    #[test]
    fn everything_becomes_opaque() {
        // 이 시험이 이 파일이 생긴 이유다. 윈도우는 투명도 자리를 0으로 준다.
        // 그대로 두면 전부 투명한 그림이 되어 아무것도 안 보인다.
        let mut b = [0u8; 8];
        bgra_to_rgba(&mut b);
        assert_eq!(b[3], 255);
        assert_eq!(b[7], 255);
    }

    #[test]
    fn a_size_of_zero_is_refused() {
        let e = save_png(std::path::Path::new("x.png"), 0, 10, &[]).unwrap_err();
        assert!(e.contains("크기가 0"), "{e}");
    }

    #[test]
    fn not_enough_pixels_is_refused_instead_of_writing_garbage() {
        // 모자란 채로 쓰면 아래쪽이 쓰레기로 채워진 그림이 나온다. 없느니만 못하다.
        let e = save_png(std::path::Path::new("x.png"), 4, 4, &[0u8; 16]).unwrap_err();
        assert!(e.contains("모자란다"), "{e}");
    }

    #[test]
    fn a_real_file_is_written() {
        let p = std::env::temp_dir().join("nabi-shot-test").join("a.png");
        let _ = std::fs::remove_file(&p);
        save_png(&p, 2, 2, &[255u8; 16]).expect("저장돼야 한다");
        assert!(std::fs::metadata(&p).is_ok_and(|m| m.len() > 0), "빈 파일이면 안 된다");
        let _ = std::fs::remove_file(&p);
    }
}
