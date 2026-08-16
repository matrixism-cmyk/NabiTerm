//! nabiPad 추출 변환 — 텍스트에서 URL/이메일을 뽑아 한 줄에 하나씩(EmEditor "extract matches"식).
//! 순수 함수. 중복 제거, 등장 순서 유지. "추출" 서브메뉴로 노출.

use nabi_i18n::{tr, Lang};

/// 등장 순서를 유지하며 중복을 제거한다.
fn dedup_keep_order(items: Vec<String>) -> String {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().filter(|s| seen.insert(s.clone())).collect::<Vec<_>>().join("\n")
}

/// 텍스트에서 http(s)://·ftp:// URL을 뽑는다(공백·따옴표·괄호에서 끊김).
pub fn extract_urls(t: &str) -> String {
    let mut out = Vec::new();
    for sch in ["https://", "http://", "ftp://"] {
        let mut s = t;
        while let Some(i) = s.find(sch) {
            let rest = &s[i..];
            let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | ')' | ']' | '}')).unwrap_or(rest.len());
            out.push(rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']).to_string());
            s = &rest[end.max(1)..];
        }
    }
    dedup_keep_order(out)
}

/// 텍스트에서 이메일 주소를 뽑는다(user@host.tld). 도메인에 점 + 끝 라벨 2+ 알파.
pub fn extract_emails(t: &str) -> String {
    let out: Vec<String> = t
        .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | ',' | ';'))
        .filter_map(|w| {
            let w = w.trim_end_matches('.');
            let (local, domain) = w.split_once('@')?;
            let last = domain.rsplit('.').next().unwrap_or("");
            let ok = !local.is_empty()
                && local.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
                && domain.contains('.')
                && last.len() >= 2
                && last.chars().all(|c| c.is_ascii_alphabetic());
            ok.then(|| w.to_string())
        })
        .collect();
    dedup_keep_order(out)
}

/// 텍스트에서 숫자(정수·소수) 토큰을 뽑는다(한 줄에 하나, 중복 제거·등장 순서).
pub fn extract_numbers(t: &str) -> String {
    let (mut out, mut cur) = (Vec::new(), String::new());
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if cur.chars().any(|c| c.is_ascii_digit()) { out.push(cur.trim_matches('.').to_string()); }
        cur.clear();
    };
    for c in t.chars() {
        if c.is_ascii_digit() || c == '.' { cur.push(c); } else { flush(&mut cur, &mut out); }
    }
    flush(&mut cur, &mut out);
    dedup_keep_order(out)
}

/// 유효한 IPv4(0~255 네 옥텟)인지.
fn is_ipv4(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 4 && p.iter().all(|o| !o.is_empty() && o.len() <= 3 && o.parse::<u32>().map(|n| n <= 255).unwrap_or(false))
}

/// 텍스트에서 IPv4 주소를 뽑는다(한 줄에 하나, 중복 제거).
pub fn extract_ips(t: &str) -> String {
    dedup_keep_order(t.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|w| is_ipv4(w)).map(|w| w.to_string()).collect())
}

/// 텍스트에서 IPv6 주소를 뽑는다(std Ipv6Addr 파서로 모든 표기[::, 전체형] 검증, 중복 제거).
pub fn extract_ipv6(t: &str) -> String {
    dedup_keep_order(
        t.split(|c: char| !(c.is_ascii_hexdigit() || c == ':'))
            .filter(|w| w.contains(':') && w.parse::<std::net::Ipv6Addr>().is_ok())
            .map(|w| w.to_string())
            .collect(),
    )
}

/// 텍스트에서 날짜/타임스탬프(ISO 8601: `YYYY-MM-DD`[`T`/공백 `HH:MM[:SS]`])를 뽑는다(로그 분석용).
pub fn extract_dates(t: &str) -> String {
    match regex::Regex::new(r"\d{4}-\d{2}-\d{2}([ T]\d{2}:\d{2}(:\d{2})?)?") {
        Ok(re) => dedup_keep_order(re.find_iter(t).map(|m| m.as_str().to_string()).collect()),
        Err(_) => String::new(),
    }
}

/// 공백·구두점으로 토큰화하고 접두문자(pfx)로 시작하는 토큰만(뒤 구두점 제거).
fn prefixed(t: &str, pfx: char) -> Vec<String> {
    t.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | ',' | ';'))
        .filter(|w| w.starts_with(pfx) && w.len() > 1)
        .map(|w| w.trim_end_matches(['.', ',', '!', '?', ':']).to_string())
        .collect()
}

/// 해시태그(#word) 추출.
pub fn extract_hashtags(t: &str) -> String {
    dedup_keep_order(prefixed(t, '#').into_iter().filter(|w| w[1..].chars().all(|c| c.is_alphanumeric() || c == '_')).collect())
}

/// 멘션(@word) 추출(이메일은 @로 시작하지 않아 제외됨).
pub fn extract_mentions(t: &str) -> String {
    dedup_keep_order(prefixed(t, '@').into_iter().filter(|w| w[1..].chars().all(|c| c.is_alphanumeric() || c == '_')).collect())
}

/// HEX 색상(#rgb/#rrggbb 등 3·4·6·8자리) 추출.
pub fn extract_hexcolors(t: &str) -> String {
    dedup_keep_order(prefixed(t, '#').into_iter().filter(|w| { let b = &w[1..]; matches!(b.len(), 3 | 4 | 6 | 8) && b.chars().all(|c| c.is_ascii_hexdigit()) }).collect())
}

/// MAC 주소(xx:xx:.. 또는 xx-xx-.. 6옥텟) 추출.
pub fn extract_macs(t: &str) -> String {
    let is_mac = |s: &str| {
        let p: Vec<&str> = if s.contains(':') { s.split(':').collect() } else { s.split('-').collect() };
        p.len() == 6 && p.iter().all(|o| o.len() == 2 && o.chars().all(|c| c.is_ascii_hexdigit()))
    };
    dedup_keep_order(t.split(|c: char| !(c.is_ascii_hexdigit() || c == ':' || c == '-')).filter(|w| is_mac(w)).map(|w| w.to_string()).collect())
}

/// 큰따옴표로 감싼 문자열 추출(따옴표 제외 내용).
pub fn extract_quoted(t: &str) -> String {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for c in t.chars() {
        match (&mut cur, c) {
            (Some(s), '"') => { out.push(std::mem::take(s)); cur = None; }
            (Some(s), _) => s.push(c),
            (None, '"') => cur = Some(String::new()),
            (None, _) => {}
        }
    }
    dedup_keep_order(out)
}

/// "추출" 서브메뉴 — 클릭한 변환 함수를 돌려준다.
pub fn extract_menu(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    for (key, f) in [
        ("editor.exthashtags", extract_hashtags as fn(&str) -> String),
        ("editor.extdates", extract_dates),
        ("editor.extmentions", extract_mentions),
        ("editor.exthexcolors", extract_hexcolors),
        ("editor.extmacs", extract_macs),
        ("editor.extquoted", extract_quoted),
    ] {
        if ui.button(tr(lang, key)).clicked() {
            ui.close();
            return Some(f);
        }
    }
    extract_menu_base(ui, lang)
}

/// 기존 추출 항목(URL·이메일·숫자·IP).
fn extract_menu_base(ui: &mut egui::Ui, lang: Lang) -> Option<fn(&str) -> String> {
    if ui.button(tr(lang, "editor.exturls")).clicked() {
        ui.close();
        return Some(extract_urls);
    }
    if ui.button(tr(lang, "editor.extemails")).clicked() {
        ui.close();
        return Some(extract_emails);
    }
    if ui.button(tr(lang, "editor.extnumbers")).clicked() {
        ui.close();
        return Some(extract_numbers);
    }
    if ui.button(tr(lang, "editor.extips")).clicked() {
        ui.close();
        return Some(extract_ips);
    }
    if ui.button(tr(lang, "editor.extipv6")).clicked() {
        ui.close();
        return Some(extract_ipv6);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{extract_emails, extract_ips, extract_ipv6, extract_numbers, extract_urls};

    #[test]
    fn extracts_urls_dedup() {
        let t = "see https://a.com/x and http://b.org. also https://a.com/x";
        assert_eq!(extract_urls(t), "https://a.com/x\nhttp://b.org");
    }

    #[test]
    fn extracts_emails() {
        let t = "me@x.com, you@sub.co.kr; not-an-email host@nodot";
        assert_eq!(extract_emails(t), "me@x.com\nyou@sub.co.kr");
    }

    #[test]
    fn extracts_numbers_and_ips() {
        assert_eq!(extract_numbers("port 8080, v3.5 x8080"), "8080\n3.5"); // 중복 8080 제거.
        assert_eq!(extract_ips("ok 192.168.0.1 bad 10.0.0.256 ok 8.8.8.8"), "192.168.0.1\n8.8.8.8");
    }

    #[test]
    fn extracts_dates() {
        let t = "start 2026-06-23 14:05:09 done 2026-06-24 again 2026-06-23 14:05:09";
        assert_eq!(super::extract_dates(t), "2026-06-23 14:05:09\n2026-06-24"); // 중복 제거.
    }

    #[test]
    fn extracts_ipv6() {
        // 압축형(::)·전체형 모두, 대괄호/포트는 분리되어 주소만, IPv4는 제외.
        let t = "host [2001:db8::1]:22 and fe80::1 plus ::1 nope 192.168.0.1";
        assert_eq!(extract_ipv6(t), "2001:db8::1\nfe80::1\n::1");
    }

    #[test]
    fn extracts_tags_colors_macs_quoted() {
        use super::{extract_hashtags, extract_hexcolors, extract_macs, extract_mentions, extract_quoted};
        assert_eq!(extract_hashtags("hi #rust and #web! #rust"), "#rust\n#web");
        assert_eq!(extract_mentions("cc @ann, @bob a@b.com"), "@ann\n@bob");
        assert_eq!(extract_hexcolors("bg #fff fg #1a2b3c nope #xyz"), "#fff\n#1a2b3c");
        assert_eq!(extract_macs("mac 00:1A:2b:3C:4d:5E x"), "00:1A:2b:3C:4d:5E");
        assert_eq!(extract_quoted("say \"hi\" and \"bye\""), "hi\nbye");
    }
}
