//! nabiPad 텍스트 도구(CyberChef 벤치마킹) — 뒤집기·공백제거·슬러그·Atbash·HTML태그제거·HEX덤프.
//! 모두 순수 함수. "텍스트 도구" 서브메뉴로 노출.

use nabi_i18n::{tr, Lang};

/// 전체 문자열을 문자 단위로 뒤집는다.
pub(crate) fn reverse_all(t: &str) -> String {
    t.chars().rev().collect()
}

/// 모든 공백 문자(스페이스/탭/개행 등)를 제거한다.
pub(crate) fn remove_whitespace(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 비ASCII 문자를 제거한다(개행 등 ASCII 제어는 보존). 텍스트를 7비트 ASCII로 정리.
pub(crate) fn ascii_only(t: &str) -> String {
    t.chars().filter(char::is_ascii).collect()
}

/// 각 줄을 감싼 따옴표(`"..."`/`'...'`)를 제거한다(짝이 맞을 때만). 목록 quote_each의 역.
pub(crate) fn dequote(t: &str) -> String {
    t.split('\n')
        .map(|l| {
            let s = l.trim();
            let q = s.starts_with('"') && s.ends_with('"') || s.starts_with('\'') && s.ends_with('\'');
            if q && s.chars().count() >= 2 {
                s[1..s.len() - 1].to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 각 줄을 URL 슬러그로(소문자 영숫자, 그 외는 `-` 한 개, 양끝 `-` 제거).
pub(crate) fn slugify(t: &str) -> String {
    t.split('\n')
        .map(|line| {
            let mut out = String::new();
            let mut dash = false;
            for c in line.chars() {
                if c.is_ascii_alphanumeric() {
                    out.extend(c.to_lowercase());
                    dash = false;
                } else if !out.is_empty() && !dash {
                    out.push('-');
                    dash = true;
                }
            }
            out.trim_end_matches('-').to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Atbash 암호(A↔Z, a↔z). 두 번 적용하면 원문.
pub(crate) fn atbash(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'A'..='Z' => (b'Z' - (c as u8 - b'A')) as char,
            'a'..='z' => (b'z' - (c as u8 - b'a')) as char,
            _ => c,
        })
        .collect()
}

/// HTML 태그(`<...>`)를 제거하고 엔티티를 디코드한다.
pub(crate) fn strip_html(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut in_tag = false;
    for c in t.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    crate::editorconvert::html_decode(&out)
}

/// ANSI 이스케이프(색·커서 등 CSI/OSC) 제거 — 터미널 출력/로그를 평문으로 정리.
pub(crate) fn strip_ansi(t: &str) -> String {
    let b = t.as_bytes();
    let mut out = String::with_capacity(t.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != 0x1b {
            let c = t[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        i += 1; // ESC
        match b.get(i) {
            Some(b'[') => {
                i += 1; // CSI: 파라미터 후 최종바이트(0x40~0x7E).
                while i < b.len() && !(0x40..=0x7e).contains(&b[i]) {
                    i += 1;
                }
                i += 1;
            }
            Some(b']') => {
                i += 1; // OSC: BEL 또는 ST(ESC\)까지.
                while i < b.len() && b[i] != 0x07 && b[i] != 0x1b {
                    i += 1;
                }
                i += if b.get(i) == Some(&0x1b) { 2 } else { 1 };
            }
            _ => i += 1, // 기타 2바이트 이스케이프.
        }
    }
    out
}

/// 각 줄을 폭 80에 맞춰 단어 단위로 줄바꿈.
pub(crate) fn hard_wrap_80(t: &str) -> String {
    t.split('\n').map(|l| wrap_line(l, 80)).collect::<Vec<_>>().join("\n")
}

fn wrap_line(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        return line.to_string();
    }
    let (mut out, mut cur) = (String::new(), 0usize);
    for word in line.split(' ') {
        let wl = word.chars().count();
        if cur > 0 && cur + 1 + wl > width {
            out.push('\n');
            cur = 0;
        } else if cur > 0 {
            out.push(' ');
            cur += 1;
        }
        out.push_str(word);
        cur += wl;
    }
    out
}

/// 문단(빈 줄로 구분) 안의 줄들을 한 줄로 합친다(줄바꿈 풀기).
pub(crate) fn unwrap_paragraphs(t: &str) -> String {
    let (mut paras, mut cur): (Vec<String>, Vec<&str>) = (Vec::new(), Vec::new());
    for line in t.split('\n') {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                paras.push(cur.join(" "));
                cur.clear();
            }
        } else {
            cur.push(line.trim());
        }
    }
    if !cur.is_empty() {
        paras.push(cur.join(" "));
    }
    paras.join("\n\n")
}

/// xxd식 HEX 덤프(offset · 16바이트 hex · ASCII).
pub(crate) fn hex_dump(t: &str) -> String {
    let mut out = String::new();
    for (i, chunk) in t.as_bytes().chunks(16).enumerate() {
        let mut hex = chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
        while hex.len() < 47 {
            hex.push(' '); // 16바이트 폭(2*16 + 15)까지 정렬.
        }
        let ascii: String = chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
        out.push_str(&format!("{:08x}  {hex}  {ascii}\n", i * 16));
    }
    out
}

/// "텍스트 도구" 서브메뉴 — 정리/폭·줄바꿈/분석·서식 하위 그룹으로 분류(2단계 계층).
pub(crate) fn texttools_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    use crate::editmenugroups::pick;
    let mut picked = None;
    ui.menu_button(tr(lang, "editor.txtclean"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.reverseall", reverse_all), ("editor.rmws", remove_whitespace),
            ("editor.asciionly", ascii_only), ("editor.dequote", dequote),
            ("editor.slugify", slugify), ("editor.striphtml", strip_html),
            ("editor.stripansi", strip_ansi),
        ]));
    });
    ui.menu_button(tr(lang, "editor.txtwrap"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.hardwrap", hard_wrap_80), ("editor.unwrap", unwrap_paragraphs),
            ("editor.full2half", crate::editorwidth::full_to_half),
            ("editor.half2full", crate::editorwidth::half_to_full),
        ]));
    });
    ui.menu_button(tr(lang, "editor.txtanalyze"), |ui| {
        picked = picked.or(pick(ui, lang, &[
            ("editor.wordfreq", crate::editorfreq::word_frequency),
            ("editor.wordlist", crate::editorfreq::word_list),
            ("editor.aligneq", crate::editoralign::align_equals),
            ("editor.aligncols", crate::editoralign::align_columns),
            ("editor.hexdump", hex_dump),
            ("editor.thousandsadd", crate::editorcodec3::thousands_add),
            ("editor.thousandsrm", crate::editorcodec3::thousands_remove),
        ]));
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tools() {
        assert_eq!(reverse_all("abc"), "cba");
        assert_eq!(remove_whitespace(" a b\tc\n"), "abc");
        assert_eq!(ascii_only("café 한글 ok"), "caf  ok"); // 비ASCII 제거(공백 보존).
        assert_eq!(dequote("\"a\"\n'b'\nc"), "a\nb\nc"); // 따옴표 짝 제거.
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(atbash(&atbash("Hello")), "Hello");
        assert_eq!(atbash("abc"), "zyx");
    }

    #[test]
    fn ansi_wrap_unwrap() {
        assert_eq!(strip_ansi("\x1b[31mRed\x1b[0m!"), "Red!");
        assert_eq!(strip_ansi("a\x1b]0;title\x07b"), "ab"); // OSC도 제거.
        assert_eq!(super::wrap_line("aa bb cc dd", 5), "aa bb\ncc dd"); // 폭 5 단어 줄바꿈.
        assert_eq!(unwrap_paragraphs("a\nb\n\nc\nd"), "a b\n\nc d");
    }

    #[test]
    fn html_and_dump() {
        assert_eq!(strip_html("<b>Hi &amp; bye</b>"), "Hi & bye");
        let d = hex_dump("Hi");
        assert!(d.starts_with("00000000  48 69"));
        assert!(d.trim_end().ends_with("  Hi"));
    }
}
