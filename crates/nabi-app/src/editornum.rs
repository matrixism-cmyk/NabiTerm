//! nabiPad 숫자 변환(Windows 계산기 프로그래머 모드/CyberChef 벤치마킹) — 진법·로마·바이트 크기.
//! 줄 단위로 적용(한 줄에 값 하나). 변환 실패한 줄은 원문 유지. "숫자" 서브메뉴.

use nabi_i18n::{tr, Lang};

/// 각 줄을 변환기로 처리(빈 줄/실패 줄은 원문 유지).
fn per_line(t: &str, f: impl Fn(&str) -> Option<String>) -> String {
    t.split('\n')
        .map(|l| {
            let s = l.trim();
            if s.is_empty() {
                l.to_string()
            } else {
                f(s).unwrap_or_else(|| l.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_radix(s: &str) -> &str {
    s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).or_else(|| s.strip_prefix("0b")).or_else(|| s.strip_prefix("0o")).unwrap_or(s)
}

pub(crate) fn dec_to_hex(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(|n| format!("{n:x}")))
}
pub(crate) fn hex_to_dec(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(strip_radix(s), 16).ok().map(|n| n.to_string()))
}
pub(crate) fn dec_to_bin(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(|n| format!("{n:b}")))
}
pub(crate) fn bin_to_dec(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(strip_radix(s), 2).ok().map(|n| n.to_string()))
}
pub(crate) fn dec_to_oct(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(|n| format!("{n:o}")))
}
pub(crate) fn oct_to_dec(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(strip_radix(s), 8).ok().map(|n| n.to_string()))
}
pub(crate) fn hex_to_bin(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(strip_radix(s), 16).ok().map(|n| format!("{n:b}")))
}
pub(crate) fn bin_to_hex(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(strip_radix(s), 2).ok().map(|n| format!("{n:x}")))
}

fn to_base36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut s = Vec::new();
    while n > 0 {
        s.push(D[(n % 36) as usize]);
        n /= 36;
    }
    s.reverse();
    String::from_utf8_lossy(&s).into_owned() // ASCII 표라 무손실 — unwrap 대신(T4-1).
}

pub(crate) fn dec_to_base36(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(to_base36))
}
pub(crate) fn base36_to_dec(t: &str) -> String {
    per_line(t, |s| u64::from_str_radix(s.trim(), 36).ok().map(|n| n.to_string()))
}

const B62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn to_base62(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut s = Vec::new();
    while n > 0 {
        s.push(B62[(n % 62) as usize]);
        n /= 62;
    }
    s.reverse();
    String::from_utf8_lossy(&s).into_owned() // ASCII 표라 무손실 — unwrap 대신(T4-1).
}

fn from_base62(s: &str) -> Option<u64> {
    let mut n: u64 = 0;
    for c in s.trim().chars() {
        let p = B62.iter().position(|&x| x as char == c)?;
        n = n.checked_mul(62)?.checked_add(p as u64)?;
    }
    Some(n)
}

pub(crate) fn dec_to_base62(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(to_base62))
}
pub(crate) fn base62_to_dec(t: &str) -> String {
    per_line(t, |s| from_base62(s).map(|n| n.to_string()))
}

fn to_roman(mut n: u32) -> Option<String> {
    if n == 0 || n > 3999 {
        return None;
    }
    const V: [(u32, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut s = String::new();
    for (v, sym) in V {
        while n >= v {
            s.push_str(sym);
            n -= v;
        }
    }
    Some(s)
}

fn from_roman(s: &str) -> Option<u32> {
    let val = |c: char| match c.to_ascii_uppercase() {
        'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50, 'C' => 100, 'D' => 500, 'M' => 1000, _ => 0,
    };
    let v: Vec<u32> = s.chars().map(val).collect();
    if v.is_empty() || v.contains(&0) {
        return None;
    }
    let mut total = 0i64;
    for i in 0..v.len() {
        if i + 1 < v.len() && v[i] < v[i + 1] {
            total -= i64::from(v[i]);
        } else {
            total += i64::from(v[i]);
        }
    }
    let r = u32::try_from(total).ok()?;
    // 왕복으로 정규형 검증(잘못된 로마 표기 거부).
    (to_roman(r).as_deref() == Some(s.to_uppercase().as_str())).then_some(r)
}

pub(crate) fn dec_to_roman(t: &str) -> String {
    per_line(t, |s| s.parse::<u32>().ok().and_then(to_roman))
}
pub(crate) fn roman_to_dec(t: &str) -> String {
    per_line(t, |s| from_roman(s).map(|n| n.to_string()))
}

/// IEEE754 단정밀도(f32) 값 → 32비트 16진 표현(예: 1.0→3f800000).
pub(crate) fn float_to_hex(t: &str) -> String {
    per_line(t, |s| s.parse::<f32>().ok().map(|f| format!("{:08x}", f.to_bits())))
}
/// 32비트 16진 → f32 값(예: 3f800000→1).
pub(crate) fn hex_to_float(t: &str) -> String {
    per_line(t, |s| u32::from_str_radix(strip_radix(s), 16).ok().map(|b| f32::from_bits(b).to_string()))
}

/// 소수점 꼬리 0을 정리한 실수 표기(50.0→"50", 0.5→"0.5").
fn trim_num(v: f64) -> String {
    let s = format!("{v:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 초 → `H:MM:SS`(1시간 미만은 `M:SS`).
pub(crate) fn sec_to_hms(t: &str) -> String {
    per_line(t, |s| {
        let total = s.parse::<u64>().ok()?;
        let (h, m, sec) = (total / 3600, total / 60 % 60, total % 60);
        Some(if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") })
    })
}
/// `H:MM:SS`/`M:SS`/`S` → 초.
pub(crate) fn hms_to_sec(t: &str) -> String {
    per_line(t, |s| {
        let mut total: u64 = 0;
        for part in s.split(':') {
            total = total.checked_mul(60)?.checked_add(part.trim().parse::<u64>().ok()?)?;
        }
        Some(total.to_string())
    })
}

/// 바이트 수 → 비트 수(×8).
pub(crate) fn bytes_to_bits(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().and_then(|n| n.checked_mul(8)).map(|v| v.to_string()))
}
/// 비트 수 → 바이트 수(÷8, 내림).
pub(crate) fn bits_to_bytes(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(|n| (n / 8).to_string()))
}

/// 정수 → 영어 서수(1→1st, 2→2nd, 3→3rd, 11→11th, 21→21st).
pub(crate) fn to_ordinal(t: &str) -> String {
    per_line(t, |s| {
        s.parse::<u64>().ok().map(|n| {
            let suffix = match (n % 100, n % 10) {
                (11..=13, _) => "th",
                (_, 1) => "st",
                (_, 2) => "nd",
                (_, 3) => "rd",
                _ => "th",
            };
            format!("{n}{suffix}")
        })
    })
}

/// 섭씨 → 화씨.
pub(crate) fn celsius_to_fahrenheit(t: &str) -> String {
    per_line(t, |s| s.parse::<f64>().ok().map(|c| trim_num(c * 9.0 / 5.0 + 32.0)))
}
/// 화씨 → 섭씨.
pub(crate) fn fahrenheit_to_celsius(t: &str) -> String {
    per_line(t, |s| s.parse::<f64>().ok().map(|f| trim_num((f - 32.0) * 5.0 / 9.0)))
}

/// "50%"→"0.5"(백분율→소수).
pub(crate) fn percent_to_decimal(t: &str) -> String {
    per_line(t, |s| s.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| trim_num(v / 100.0)))
}
/// "0.5"→"50%"(소수→백분율).
pub(crate) fn decimal_to_percent(t: &str) -> String {
    per_line(t, |s| s.parse::<f64>().ok().map(|v| format!("{}%", trim_num(v * 100.0))))
}

fn human(n: u64) -> String {
    const U: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let (mut v, mut i) = (n as f64, 0);
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.2} {}", U[i])
    }
}

fn parse_human(s: &str) -> Option<u64> {
    let pos = s.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = s.split_at(pos);
    let num: f64 = num.trim().parse().ok()?;
    let mult = match unit.trim().to_uppercase().as_str() {
        "B" => 1.0,
        "KB" | "K" => 1024f64,
        "MB" | "M" => 1024f64.powi(2),
        "GB" | "G" => 1024f64.powi(3),
        "TB" | "T" => 1024f64.powi(4),
        "PB" => 1024f64.powi(5),
        _ => return None,
    };
    Some((num * mult) as u64)
}

pub(crate) fn bytes_human(t: &str) -> String {
    per_line(t, |s| s.parse::<u64>().ok().map(human))
}
pub(crate) fn human_bytes(t: &str) -> String {
    per_line(t, |s| parse_human(s).map(|n| n.to_string()))
}

/// "숫자" 서브메뉴 — 클릭한 변환 함수를 돌려준다.
pub(crate) fn num_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editmenugroups::pick;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.basegroup"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.dec2hex", dec_to_hex), ("editor.hex2dec", hex_to_dec),
            ("editor.dec2bin", dec_to_bin), ("editor.bin2dec", bin_to_dec),
            ("editor.dec2oct", dec_to_oct), ("editor.oct2dec", oct_to_dec),
            ("editor.hex2bin", hex_to_bin), ("editor.bin2hex", bin_to_hex),
            ("editor.dec2base36", dec_to_base36), ("editor.base362dec", base36_to_dec),
            ("editor.dec2base62", dec_to_base62), ("editor.base622dec", base62_to_dec),
        ]));
    });
    ui.menu_button(tr(lang, "editor.numspecial"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.dec2roman", dec_to_roman), ("editor.roman2dec", roman_to_dec),
            ("editor.byteshuman", bytes_human), ("editor.humanbytes", human_bytes),
            ("editor.float2hex", float_to_hex), ("editor.hex2float", hex_to_float),
            ("editor.pct2dec", percent_to_decimal), ("editor.dec2pct", decimal_to_percent),
            ("editor.sec2hms", sec_to_hms), ("editor.hms2sec", hms_to_sec),
            ("editor.c2f", celsius_to_fahrenheit), ("editor.f2c", fahrenheit_to_celsius),
            ("editor.ordinal", to_ordinal),
            ("editor.bytes2bits", bytes_to_bits), ("editor.bits2bytes", bits_to_bytes),
            ("editor.numinc", crate::editornumops::increment_numbers), ("editor.numdec", crate::editornumops::decrement_numbers),
        ]));
    });
    picked
}

