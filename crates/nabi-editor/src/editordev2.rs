//! nabiPad 개발 도구(2) — 색상 HEX↔RGB·CRC32·SQL/정규식 이스케이프·IP↔정수. 순수 함수.

/// `#RRGGBB`/`RGB`/`RRGGBB` → `rgb(r, g, b)`. 실패 시 원문.
pub fn hex_color_to_rgb(t: &str) -> String {
    let h = t.trim().trim_start_matches('#');
    let hex = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return format!("rgb({r}, {g}, {b})");
        }
    }
    t.to_string()
}

/// `rgb(r,g,b)`/`r,g,b` → `#RRGGBB`. 실패 시 원문.
pub fn rgb_to_hex_color(t: &str) -> String {
    let inner = t.trim().trim_start_matches("rgb").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<&str> = inner.split(',').map(str::trim).collect();
    if n.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (n[0].parse::<u8>(), n[1].parse::<u8>(), n[2].parse::<u8>()) {
            return format!("#{r:02X}{g:02X}{b:02X}");
        }
    }
    t.to_string()
}

/// `#RRGGBB`/`RGB`/`RRGGBB` → `hsl(h, s%, l%)`(CSS). 실패 시 원문.
pub fn hex_to_hsl(t: &str) -> String {
    let h = t.trim().trim_start_matches('#');
    let hex: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    let parse = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let (Some(r), Some(g), Some(b)) = (parse(0), parse(2), parse(4)) else { return t.to_string() };
    let (rf, gf, bf) = (f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (mut hue, sat) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let hh = if max == rf {
            ((gf - bf) / d).rem_euclid(6.0)
        } else if max == gf {
            (bf - rf) / d + 2.0
        } else {
            (rf - gf) / d + 4.0
        };
        (hh * 60.0, s)
    };
    if hue < 0.0 {
        hue += 360.0;
    }
    format!("hsl({}, {}%, {}%)", hue.round() as i64, (sat * 100.0).round() as i64, (l * 100.0).round() as i64)
}

/// `hsl(h, s%, l%)` → `#RRGGBB`(CSS). 실패 시 원문.
pub fn hsl_to_hex(t: &str) -> String {
    let inner = t.trim().trim_start_matches("hsl").trim().trim_start_matches('(').trim_end_matches(')');
    let n: Vec<&str> = inner.split(',').map(|x| x.trim().trim_end_matches('%').trim()).collect();
    if n.len() != 3 {
        return t.to_string();
    }
    let (Ok(h), Ok(s), Ok(l)) = (n[0].parse::<f64>(), n[1].parse::<f64>(), n[2].parse::<f64>()) else {
        return t.to_string();
    };
    let (s, l) = (s / 100.0, l / 100.0);
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
    format!("#{:02X}{:02X}{:02X}", to(r1), to(g1), to(b1))
}

/// CRC-32(IEEE) 체크섬을 8자리 대문자 16진으로.
pub fn crc32_hex(t: &str) -> String {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in t.as_bytes() {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    format!("{:08X}", crc ^ 0xFFFF_FFFF)
}

/// CRC-32을 소문자 16진으로(해시 메뉴 일관성).
pub fn crc32_lower(t: &str) -> String {
    crc32_hex(t).to_lowercase()
}

/// SQL 문자열 리터럴로 이스케이프(`'`→`''`, 양끝 작은따옴표).
pub fn sql_escape(t: &str) -> String {
    format!("'{}'", t.replace('\'', "''"))
}

/// 정규식 메타문자 이스케이프(리터럴 검색용).
pub fn regex_escape(t: &str) -> String {
    regex::escape(t.trim_end_matches('\n'))
}

/// 점표기 IPv4 → 32비트 정수. 실패 시 원문.
pub fn ip_to_int(t: &str) -> String {
    let p: Vec<&str> = t.trim().split('.').collect();
    if p.len() == 4 {
        let o: Option<Vec<u32>> = p.iter().map(|x| x.parse::<u8>().ok().map(u32::from)).collect();
        if let Some(o) = o {
            return ((o[0] << 24) | (o[1] << 16) | (o[2] << 8) | o[3]).to_string();
        }
    }
    t.to_string()
}

/// 32비트 정수 → 점표기 IPv4. 실패 시 원문.
pub fn int_to_ip(t: &str) -> String {
    match t.trim().parse::<u32>() {
        Ok(n) => format!("{}.{}.{}.{}", n >> 24, (n >> 16) & 0xFF, (n >> 8) & 0xFF, n & 0xFF),
        Err(_) => t.to_string(),
    }
}

/// JSON → TOML(데이터 형식 변환). 실패 시 원문.
pub fn json_to_toml(t: &str) -> String {
    serde_json::from_str::<serde_json::Value>(t).ok().and_then(|v| toml::to_string_pretty(&v).ok()).unwrap_or_else(|| t.to_string())
}

/// TOML → JSON. 실패 시 원문.
pub fn toml_to_json(t: &str) -> String {
    toml::from_str::<serde_json::Value>(t).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| t.to_string())
}

/// JSON 객체 키를 (재귀적으로) 사전순 정렬해 정형 출력한다(serde_json BTreeMap). 실패 시 원문.
pub fn json_sort_keys(t: &str) -> String {
    serde_json::from_str::<serde_json::Value>(t).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| t.to_string())
}

/// 각 줄을 JSON 문자열 배열로 만든다(`a\nb` → `["a","b"]`). 따옴표/특수문자는 serde가 이스케이프.
pub fn lines_to_json_array(t: &str) -> String {
    let arr: Vec<serde_json::Value> = t.lines().map(|l| serde_json::Value::String(l.to_string())).collect();
    serde_json::to_string(&serde_json::Value::Array(arr)).unwrap_or_else(|_| t.to_string())
}

/// JSON 배열을 한 줄에 하나씩 펼친다(문자열은 따옴표 없이, 그 외는 압축 JSON). 실패 시 원문.
pub fn json_array_to_lines(t: &str) -> String {
    match serde_json::from_str::<Vec<serde_json::Value>>(t) {
        Ok(arr) => arr
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(_) => t.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert_eq!(hex_color_to_rgb("#FF8000"), "rgb(255, 128, 0)");
        assert_eq!(hex_color_to_rgb("#abc"), "rgb(170, 187, 204)"); // 단축형.
        assert_eq!(rgb_to_hex_color("rgb(255, 128, 0)"), "#FF8000");
        assert_eq!(rgb_to_hex_color("0,0,0"), "#000000");
        assert_eq!(hex_to_hsl("#ff0000"), "hsl(0, 100%, 50%)");
        assert_eq!(hex_to_hsl("#808080"), "hsl(0, 0%, 50%)"); // 무채색.
        assert_eq!(hex_to_hsl("nope"), "nope");
        assert_eq!(hsl_to_hex("hsl(0, 100%, 50%)"), "#FF0000");
        assert_eq!(hsl_to_hex("hsl(120, 100%, 50%)"), "#00FF00");
        assert_eq!(hsl_to_hex("bad"), "bad");
    }

    #[test]
    fn crc_and_escapes() {
        assert_eq!(crc32_hex("123456789"), "CBF43926"); // 표준 검사값.
        assert_eq!(crc32_hex(""), "00000000");
        assert_eq!(sql_escape("O'Brien"), "'O''Brien'");
        assert_eq!(regex_escape("a.b*c"), "a\\.b\\*c");
    }

    #[test]
    fn ip_roundtrip() {
        assert_eq!(ip_to_int("192.168.0.1"), "3232235521");
        assert_eq!(int_to_ip("3232235521"), "192.168.0.1");
        assert_eq!(ip_to_int("not.an.ip.x"), "not.an.ip.x");
    }

    #[test]
    fn json_keys_sorted() {
        assert_eq!(json_sort_keys("{\"b\":1,\"a\":2}"), "{\n  \"a\": 2,\n  \"b\": 1\n}");
        assert_eq!(json_sort_keys("nope"), "nope"); // 잘못된 JSON → 원문.
    }

    #[test]
    fn lines_json_array_roundtrip() {
        assert_eq!(lines_to_json_array("a\nb\nc"), "[\"a\",\"b\",\"c\"]");
        assert_eq!(lines_to_json_array("a\"b\tc"), "[\"a\\\"b\\tc\"]"); // 특수문자 이스케이프.
        assert_eq!(json_array_to_lines("[\"a\",\"b\",\"c\"]"), "a\nb\nc");
        assert_eq!(json_array_to_lines("[1,true,\"x\"]"), "1\ntrue\nx"); // 비문자열은 그대로.
        assert_eq!(json_array_to_lines("nope"), "nope"); // 잘못된 JSON → 원문.
    }
}
