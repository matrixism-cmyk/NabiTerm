//! exe 아이콘 패치 — windres 없이 UpdateResourceW로 RT_ICON/RT_GROUP_ICON 주입.
//! (이 환경엔 MinGW windres가 없어 build.rs의 winres 임베드가 생략됨 — 사후 주입.)

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

#[link(name = "kernel32")]
extern "system" {
    fn BeginUpdateResourceW(file: *const u16, delete_existing: i32) -> isize;
    fn UpdateResourceW(
        h: isize,
        ty: *const u16,
        name: *const u16,
        lang: u16,
        data: *const u8,
        len: u32,
    ) -> i32;
    fn EndUpdateResourceW(h: isize, discard: i32) -> i32;
}

const RT_ICON: usize = 3;
const RT_GROUP_ICON: usize = 14;
const SIZES: [usize; 7] = [16, 24, 32, 48, 64, 128, 256];

/// appicon.rs/build.rs와 동일한 나비 RGBA(크레이트 경계로 복제).
fn butterfly_rgba(s: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; s * s * 4];
    let f = s as f32 / 64.0; // 64 기준 좌표 스케일.
    let wings: [(f32, f32, f32, f32); 4] = [
        (22.0, 24.0, 15.0, 12.5),
        (42.0, 24.0, 15.0, 12.5),
        (24.0, 43.0, 11.0, 9.5),
        (40.0, 43.0, 11.0, 9.5),
    ];
    for y in 0..s {
        for x in 0..s {
            let (fx, fy) = (x as f32 / f + 0.5, y as f32 / f + 0.5);
            let mut a = 0.0f32;
            for (cx, cy, rx, ry) in wings {
                let d = ((fx - cx) / rx).powi(2) + ((fy - cy) / ry).powi(2);
                if d < 1.0 {
                    a = a.max(1.0 - d * d);
                }
            }
            let i = (y * s + x) * 4;
            if a > 0.02 {
                let t = fx / 64.0;
                rgba[i] = (64.0 + 86.0 * t) as u8;
                rgba[i + 1] = (180.0 - 70.0 * t) as u8;
                rgba[i + 2] = (230.0 + 5.0 * t) as u8;
                rgba[i + 3] = (a.min(1.0) * 255.0) as u8;
            }
            let bd = ((fx - 32.0) / 2.6).powi(2) + ((fy - 33.0) / 16.0).powi(2);
            if bd < 1.0 {
                rgba[i] = 30;
                rgba[i + 1] = 36;
                rgba[i + 2] = 48;
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

/// 한 크기의 RT_ICON 페이로드(BITMAPINFOHEADER + 32bpp bottom-up + 빈 AND 마스크).
fn ico_image(size: usize) -> Vec<u8> {
    let rgba = butterfly_rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let mut img = Vec::with_capacity(40 + size * size * 4 + mask_row * size);
    img.extend_from_slice(&40u32.to_le_bytes());
    img.extend_from_slice(&(size as i32).to_le_bytes());
    img.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // 높이=2배(XOR+AND).
    img.extend_from_slice(&[1, 0, 32, 0]);
    img.extend_from_slice(&[0u8; 24]);
    for y in (0..size).rev() {
        for x in 0..size {
            let i = (y * size + x) * 4;
            img.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    img.extend(std::iter::repeat_n(0u8, mask_row * size)); // AND 마스크=0(알파 사용).
    img
}

/// exe의 아이콘 리소스를 나비 아이콘으로 교체/주입한다.
pub fn patch(exe: &Path) -> Result<(), String> {
    let images: Vec<Vec<u8>> = SIZES.iter().map(|&s| ico_image(s)).collect();
    // GRPICONDIR(+엔트리: ICONDIRENTRY에서 오프셋 대신 WORD 리소스 ID).
    let mut grp = vec![0u8, 0, 1, 0, SIZES.len() as u8, 0];
    for (i, (&s, img)) in SIZES.iter().zip(&images).enumerate() {
        let dim = if s >= 256 { 0 } else { s as u8 };
        grp.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
        grp.extend_from_slice(&(img.len() as u32).to_le_bytes());
        grp.extend_from_slice(&(i as u16 + 1).to_le_bytes());
    }
    let wide: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: wide는 NUL로 끝나는 UTF-16이고, 자원 갱신 핸들은 이 블록 안에서 열고 닫는다.
    // 실패 시 조기 반환해 열린 핸들을 남기지 않는다.
    unsafe {
        let h = BeginUpdateResourceW(wide.as_ptr(), 0);
        if h == 0 {
            return Err(format!("BeginUpdateResource 실패: {}", exe.display()));
        }
        // MAKEINTRESOURCE: 하위 16비트가 ID인 정수 포인터.
        let res_id = |id: usize| std::ptr::null::<u16>().wrapping_byte_add(id);
        for (i, img) in images.iter().enumerate() {
            let name = res_id(i + 1);
            if UpdateResourceW(h, res_id(RT_ICON), name, 1033, img.as_ptr(), img.len() as u32) == 0 {
                EndUpdateResourceW(h, 1);
                return Err(format!("RT_ICON #{} 주입 실패", i + 1));
            }
        }
        let ok = UpdateResourceW(h, res_id(RT_GROUP_ICON), res_id(1), 1033, grp.as_ptr(), grp.len() as u32);
        if ok == 0 {
            EndUpdateResourceW(h, 1);
            return Err("RT_GROUP_ICON 주입 실패".into());
        }
        if EndUpdateResourceW(h, 0) == 0 {
            return Err("EndUpdateResource 실패(파일 사용 중?)".into());
        }
    }
    Ok(())
}
