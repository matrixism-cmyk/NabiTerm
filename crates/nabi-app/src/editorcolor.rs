//! nabiPad 색상 변환 도구(CSS/CyberChef 벤치마킹) — rgb()↔hsl()·HEX 반전. 순수 함수.
//! HEX↔rgb/hsl은 editordev2에, 본 모듈은 rgb↔hsl과 반전 등 추가 변환을 모은다.

/// `rgb(r,g,b)`/`r,g,b`에서 (r,g,b) 추출.
fn parse_rgb(t: &str) -> Option<(u8, u8, u8)> {
    let inner = t.trim().trim_start_matches("rgb").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<&str> = inner.split(',').map(str::trim).collect();
    if n.len() != 3 {
        return None;
    }
    Some((n[0].parse().ok()?, n[1].parse().ok()?, n[2].parse().ok()?))
}

/// (r,g,b) → (h도, s, l) (s·l은 0~1).
fn rgb_to_hsl_core(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (rf, gf, bf) = (f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == rf {
        ((gf - bf) / d).rem_euclid(6.0)
    } else if max == gf {
        (bf - rf) / d + 2.0
    } else {
        (rf - gf) / d + 4.0
    };
    ((h * 60.0).rem_euclid(360.0), s, l)
}

/// (h도, s, l) → (r,g,b).
fn hsl_to_rgb_core(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r1), to(g1), to(b1))
}

/// `rgb(r,g,b)` → `hsl(h, s%, l%)`. 실패 시 원문.
pub(crate) fn rgb_to_hsl(t: &str) -> String {
    let Some((r, g, b)) = parse_rgb(t) else { return t.to_string() };
    let (h, s, l) = rgb_to_hsl_core(r, g, b);
    format!("hsl({}, {}%, {}%)", h.round() as i64, (s * 100.0).round() as i64, (l * 100.0).round() as i64)
}

/// `hsl(h, s%, l%)` → `rgb(r, g, b)`. 실패 시 원문.
pub(crate) fn hsl_to_rgb(t: &str) -> String {
    let inner = t.trim().trim_start_matches("hsl").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<&str> = inner.split(',').map(|x| x.trim().trim_end_matches('%').trim()).collect();
    if n.len() != 3 {
        return t.to_string();
    }
    let (Ok(h), Ok(s), Ok(l)) = (n[0].parse::<f64>(), n[1].parse::<f64>(), n[2].parse::<f64>()) else {
        return t.to_string();
    };
    let (r, g, b) = hsl_to_rgb_core(h, s / 100.0, l / 100.0);
    format!("rgb({r}, {g}, {b})")
}

/// `rgb(r,g,b)` → 휘도(Rec.601) 그레이스케일 `#GGGGGG`. 실패 시 원문.
pub(crate) fn rgb_to_grayscale(t: &str) -> String {
    let Some((r, g, b)) = parse_rgb(t) else { return t.to_string() };
    let y = (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)).round().clamp(0.0, 255.0) as u8;
    format!("#{y:02X}{y:02X}{y:02X}")
}

/// `rgb(r,g,b)` → `cmyk(c%, m%, y%, k%)`(인쇄용). 실패 시 원문.
pub(crate) fn rgb_to_cmyk(t: &str) -> String {
    let Some((r, g, b)) = parse_rgb(t) else { return t.to_string() };
    let (rf, gf, bf) = (f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
    let k = 1.0 - rf.max(gf).max(bf);
    let chan = |v: f64| if k >= 1.0 { 0.0 } else { (1.0 - v - k) / (1.0 - k) };
    let pct = |v: f64| (v * 100.0).round() as i64;
    format!("cmyk({}%, {}%, {}%, {}%)", pct(chan(rf)), pct(chan(gf)), pct(chan(bf)), pct(k))
}

/// `cmyk(c%, m%, y%, k%)` → `rgb(r, g, b)`. 실패 시 원문.
pub(crate) fn cmyk_to_rgb(t: &str) -> String {
    let inner = t.trim().trim_start_matches("cmyk").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<f64> = inner.split(',').filter_map(|x| x.trim().trim_end_matches('%').trim().parse().ok()).collect();
    if n.len() != 4 {
        return t.to_string();
    }
    let (c, m, y, k) = (n[0] / 100.0, n[1] / 100.0, n[2] / 100.0, n[3] / 100.0);
    let ch = |v: f64| (255.0 * (1.0 - v) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
    format!("rgb({}, {}, {})", ch(c), ch(m), ch(y))
}

/// `#RRGGBBAA`(8자리) → `rgba(r, g, b, a)`(a는 0~1). 실패 시 원문.
pub(crate) fn hex8_to_rgba(t: &str) -> String {
    let h = t.trim().trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    let (Some(r), Some(g), Some(b), Some(a)) = (p(0), p(2), p(4), p(6)) else { return t.to_string() };
    let af = format!("{:.3}", f64::from(a) / 255.0);
    let af = af.trim_end_matches('0').trim_end_matches('.');
    format!("rgba({r}, {g}, {b}, {af})")
}

/// `rgba(r, g, b, a)`(a 0~1) → `#RRGGBBAA`. 실패 시 원문.
pub(crate) fn rgba_to_hex8(t: &str) -> String {
    let inner = t.trim().trim_start_matches("rgba").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<&str> = inner.split(',').map(str::trim).collect();
    if n.len() != 4 {
        return t.to_string();
    }
    let (Ok(r), Ok(g), Ok(b), Ok(a)) = (n[0].parse::<u8>(), n[1].parse::<u8>(), n[2].parse::<u8>(), n[3].parse::<f64>()) else {
        return t.to_string();
    };
    let a8 = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{r:02X}{g:02X}{b:02X}{a8:02X}")
}

/// HEX 색을 `#RRGGBB`(대문자, 6자리)로 정규화한다(`#abc`/`abc`/`aabbcc` 허용). 실패 시 원문.
pub(crate) fn normalize_hex(t: &str) -> String {
    let h = t.trim().trim_start_matches('#');
    let hex: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("#{}", hex.to_uppercase())
    } else {
        t.to_string()
    }
}

/// `rgb(r,g,b)` 또는 `#RRGGBB`의 보색(색상환 반대편, 명도/채도 유지)을 `#RRGGBB`로. 실패 시 원문.
pub(crate) fn complementary(t: &str) -> String {
    // rgb() 또는 HEX 모두 허용 — HEX는 editordev2 변환을 거쳐 rgb로.
    let rgb_str = if t.trim_start().starts_with('#') { crate::editordev2::hex_color_to_rgb(t) } else { t.to_string() };
    let Some((r, g, b)) = parse_rgb(&rgb_str) else { return t.to_string() };
    let (h, s, l) = rgb_to_hsl_core(r, g, b);
    let (nr, ng, nb) = hsl_to_rgb_core((h + 180.0) % 360.0, s, l);
    format!("#{nr:02X}{ng:02X}{nb:02X}")
}

/// `#RRGGBB`/`RGB`/`RRGGBB` 색을 반전(255-각 채널)해 `#RRGGBB`로. 실패 시 원문.
pub(crate) fn invert_color(t: &str) -> String {
    let h = t.trim().trim_start_matches('#');
    let hex: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let (Some(r), Some(g), Some(b)) = (p(0), p(2), p(4)) else { return t.to_string() };
    format!("#{:02X}{:02X}{:02X}", 255 - r, 255 - g, 255 - b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsl_roundtrip() {
        assert_eq!(rgb_to_hsl("rgb(255, 0, 0)"), "hsl(0, 100%, 50%)");
        assert_eq!(hsl_to_rgb("hsl(120, 100%, 50%)"), "rgb(0, 255, 0)");
        assert_eq!(rgb_to_hsl("bad"), "bad");
    }

    #[test]
    fn rgb_cmyk() {
        assert_eq!(rgb_to_cmyk("rgb(255, 0, 0)"), "cmyk(0%, 100%, 100%, 0%)");
        assert_eq!(rgb_to_cmyk("rgb(0, 0, 0)"), "cmyk(0%, 0%, 0%, 100%)");
        assert_eq!(cmyk_to_rgb("cmyk(0%, 100%, 100%, 0%)"), "rgb(255, 0, 0)");
        assert_eq!(rgb_to_cmyk("bad"), "bad");
        assert_eq!(rgb_to_grayscale("rgb(255, 255, 255)"), "#FFFFFF");
        assert_eq!(rgb_to_grayscale("rgb(0, 0, 0)"), "#000000");
        assert_eq!(hex8_to_rgba("#FF8000FF"), "rgba(255, 128, 0, 1)");
        assert_eq!(hex8_to_rgba("#00000080"), "rgba(0, 0, 0, 0.502)");
        assert_eq!(rgba_to_hex8("rgba(255, 128, 0, 1)"), "#FF8000FF");
        assert_eq!(rgba_to_hex8("bad"), "bad");
        assert_eq!(complementary("rgb(255, 0, 0)"), "#00FFFF"); // 빨강 보색=시안.
        assert_eq!(complementary("#FF0000"), "#00FFFF"); // HEX 입력도 허용.
        assert_eq!(complementary("nope"), "nope");
        assert_eq!(normalize_hex("#abc"), "#AABBCC");
        assert_eq!(normalize_hex("ff8000"), "#FF8000");
        assert_eq!(normalize_hex("nope"), "nope");
    }

    #[test]
    fn invert() {
        assert_eq!(invert_color("#FFFFFF"), "#000000");
        assert_eq!(invert_color("#000"), "#FFFFFF"); // 단축형.
        assert_eq!(invert_color("#FF8000"), "#007FFF");
        assert_eq!(invert_color("nope"), "nope");
    }
}
