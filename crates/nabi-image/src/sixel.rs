//! Sixel(DCS) 디코더 — `<params> q <data>` 페이로드를 RGBA로 푼다.
//!
//! 데이터 문자(0x3F..0x7E)는 각 6개 세로 픽셀(비트0=위)을 인코딩한다.
//! `#n`=색 선택, `#n;Pu;Px;Py;Pz`=색 정의(Pu 2=RGB·1=HLS, 0~100), `!n`=다음 문자 n회 반복,
//! `$`=캐리지리턴(같은 밴드 왼쪽), `-`=다음 밴드(6픽셀 아래).

use crate::DecodedImage;
use std::collections::HashMap;

const MAX_W: usize = 4000;
const MAX_H: usize = 4000;

/// Sixel 페이로드(첫 'q' 이후가 데이터)를 RGBA 이미지로 디코드한다. 실패/빈 이미지면 None.
pub fn decode_sixel(payload: &[u8]) -> Option<DecodedImage> {
    let q = payload.iter().position(|&b| b == b'q')?;
    // sixel 인트로는 `[숫자;]* q` 형태 — 그 외 DCS(DECRQSS `$q` 등)는 이미지가 아니므로 제외.
    if !payload[..q].iter().all(|&b| b.is_ascii_digit() || b == b';') {
        return None;
    }
    let data = &payload[q + 1..];
    let mut pal = default_palette();
    let mut rows: Vec<Vec<u16>> = Vec::new(); // 픽셀당 색인+1 (0=비설정).
    let (mut x, mut band, mut color) = (0usize, 0usize, 0u16);
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'#' => {
                i += 1;
                let (n, ni) = parse_num(data, i);
                i = ni;
                color = n;
                let params = read_params(data, &mut i);
                if params.len() >= 4 {
                    if let Some(c) = color_def(params[0], params[1], params[2], params[3]) {
                        pal.insert(n, c);
                    }
                }
            }
            b'!' => {
                i += 1;
                let (rep, ni) = parse_num(data, i);
                i = ni;
                if let Some(&sx) = data.get(i) {
                    if (0x3F..=0x7E).contains(&sx) {
                        plot(&mut rows, &mut x, band, color, sx - 0x3F, rep.max(1));
                        i += 1;
                    }
                }
            }
            b'$' => {
                x = 0;
                i += 1;
            }
            b'-' => {
                band += 1;
                x = 0;
                i += 1;
            }
            c @ 0x3F..=0x7E => {
                plot(&mut rows, &mut x, band, color, c - 0x3F, 1);
                i += 1;
            }
            _ => i += 1, // 공백/개행 등 무시.
        }
    }
    assemble(&rows, &pal)
}

/// 한 sixel 문자(6비트)를 x열에 repeat회 찍는다(각 비트=세로 1픽셀).
fn plot(rows: &mut Vec<Vec<u16>>, x: &mut usize, band: usize, color: u16, bits: u8, repeat: u16) {
    for _ in 0..repeat {
        if *x >= MAX_W {
            break;
        }
        let y0 = band * 6;
        for k in 0..6 {
            if bits & (1 << k) != 0 && y0 + k < MAX_H {
                set(rows, *x, y0 + k, color + 1);
            }
        }
        *x += 1;
    }
}

fn set(rows: &mut Vec<Vec<u16>>, x: usize, y: usize, v: u16) {
    if rows.len() <= y {
        rows.resize(y + 1, Vec::new());
    }
    if rows[y].len() <= x {
        rows[y].resize(x + 1, 0);
    }
    rows[y][x] = v;
}

/// 픽셀 격자를 RGBA로 변환한다(비설정=투명). 빈 이미지면 None.
fn assemble(rows: &[Vec<u16>], pal: &HashMap<u16, (u8, u8, u8)>) -> Option<DecodedImage> {
    let h = rows.len();
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if w == 0 || h == 0 {
        return None;
    }
    let mut rgba = vec![0u8; w * h * 4];
    for (y, row) in rows.iter().enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let (r, g, b) = pal.get(&(v - 1)).copied().unwrap_or((255, 255, 255));
            let o = (y * w + x) * 4;
            rgba[o] = r;
            rgba[o + 1] = g;
            rgba[o + 2] = b;
            rgba[o + 3] = 255;
        }
    }
    Some(DecodedImage { width: w as u32, height: h as u32, rgba })
}

/// 위치 i에서 연속 숫자를 읽어 (값, 다음 인덱스)를 돌려준다.
fn parse_num(d: &[u8], mut i: usize) -> (u16, usize) {
    let mut n: u32 = 0;
    while let Some(&b) = d.get(i) {
        if b.is_ascii_digit() {
            n = (n * 10 + u32::from(b - b'0')).min(65535);
            i += 1;
        } else {
            break;
        }
    }
    (n as u16, i)
}

/// `;num` 형태의 파라미터들을 읽는다(색 정의용). i를 전진시킨다.
fn read_params(d: &[u8], i: &mut usize) -> Vec<u16> {
    let mut out = Vec::new();
    while d.get(*i) == Some(&b';') {
        *i += 1;
        let (n, ni) = parse_num(d, *i);
        *i = ni;
        out.push(n);
    }
    out
}

/// 색 정의 파라미터(Pu, Px, Py, Pz)를 RGB로(Pu=2 RGB 0~100, Pu=1 HLS).
fn color_def(pu: u16, px: u16, py: u16, pz: u16) -> Option<(u8, u8, u8)> {
    let s = |v: u16| ((u32::from(v.min(100)) * 255 + 50) / 100) as u8;
    match pu {
        2 => Some((s(px), s(py), s(pz))),
        1 => Some(hls_to_rgb(px, py, pz)),
        _ => None,
    }
}

/// HLS(H 0~360, L·S 0~100) → RGB. (sixel은 RGB가 흔하지만 호환용.)
fn hls_to_rgb(h: u16, l: u16, s: u16) -> (u8, u8, u8) {
    let (h, l, s) = (f32::from(h) / 360.0, f32::from(l) / 100.0, f32::from(s) / 100.0);
    if s <= 0.0 {
        let v = (l * 255.0) as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = (t + 1.0) % 1.0;
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0) as u8
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// 색 정의 전 기본 팔레트(VT340풍 16색 근사).
fn default_palette() -> HashMap<u16, (u8, u8, u8)> {
    let base = [
        (0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0),
        (0, 0, 238), (205, 0, 205), (0, 205, 205), (229, 229, 229),
        (127, 127, 127), (255, 0, 0), (0, 255, 0), (255, 255, 0),
        (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
    ];
    base.iter().enumerate().map(|(i, &c)| (i as u16, c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_single_red_column() {
        // q 뒤: 색0=빨강(RGB 100,0,0) 정의 후 '~'(6비트 모두) 1열 → 1x6 빨강.
        let img = decode_sixel(b"q#0;2;100;0;0~").expect("디코드");
        assert_eq!((img.width, img.height), (1, 6));
        assert_eq!(&img.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&img.rgba[20..24], &[255, 0, 0, 255]); // 6번째 픽셀도 빨강.
    }

    #[test]
    fn repeat_and_newband() {
        // '!3~' = '~' 3회(3열), '-' 다음 밴드, '@'(비트0만) 1열.
        let img = decode_sixel(b"q#0;2;0;100;0!3~-@").expect("디코드");
        assert_eq!(img.width, 3); // 첫 밴드가 3열로 가장 넓다.
        assert_eq!(img.height, 7); // 밴드0=6 + 밴드1의 1행.
        assert_eq!(&img.rgba[0..4], &[0, 255, 0, 255]); // 초록.
    }

    #[test]
    fn empty_payload_is_none() {
        assert!(decode_sixel(b"q").is_none()); // 'q' 뒤 데이터 없음.
        assert!(decode_sixel(b"abc").is_none()); // 'q' 자체가 없음.
    }
}
