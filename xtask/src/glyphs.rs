//! **화면에 두부(□)로 나올 글자를 센다.**
//!
//! ## 왜 필요한가
//!
//! 소스에 `\u{1f4be}`(💾) 라고 적어도, 그 글자가 어느 글꼴에도 없으면 화면에는 네모
//! 상자가 뜬다. **컴파일도 시험도 통과한다.** 눈으로 보기 전에는 아무도 모른다.
//!
//! 실제로 그렇게 됐다(2026-08-31, 화면을 찍어 확인). 파일 브라우저는 폴더든 파일이든
//! 같은 상자라 종류를 구분할 수 없었고, nabiPad 툴바는 저장·강조·줄바꿈·읽기전용이
//! 전부 상자였다. 내장 브라우저 툴바에서 한 번 겪고 "어느 PC 에서나 그려지는 모양을
//! 쓴다"고 적어 두었는데, 그 교훈이 나머지 화면에는 옮겨지지 않았다.
//!
//! ## 어떻게 세는가
//!
//! 소스의 `\u{...}` 를 모두 모으고, **우리가 실제로 까는 글꼴**(`fonts.rs` 의 후보)에
//! 그 글자가 있는지 글꼴 파일의 cmap 으로 확인한다. 짐작이 아니라 파일을 읽어 본다.
//!
//! egui 가 기본으로 넣는 글꼴(Noto Emoji 등)은 여기서 보지 않는다 — 그쪽이 덮어 주는
//! 것도 있으므로 이 검사의 결과는 **"없을 수 있다"** 이지 "반드시 두부"는 아니다.
//! 그래서 막지 않고 목록만 준다. 목록을 받아 화면으로 확인하는 것이 다음 단계다.

use std::collections::{BTreeMap, BTreeSet};

/// 우리가 까는 폴백 글꼴들 — `crates/nabi-app/src/fonts.rs` 의 후보와 같아야 한다.
const FONTS: &[(&str, &str)] = &[
    ("malgun", r"C:\Windows\Fonts\malgun.ttf"),
    ("seguisym", r"C:\Windows\Fonts\seguisym.ttf"),
];

/// 그 글꼴에 이 글자가 있는가.
fn has_glyph(data: &[u8], c: char) -> Option<bool> {
    use skrifa::MetadataProvider;
    let face = skrifa::FontRef::new(data).ok()?;
    Some(face.charmap().map(c).is_some())
}

/// 소스에서 `\u{...}` 로 적힌 글자를 모은다 — 어느 파일에서 왔는지도 함께.
fn collect() -> BTreeMap<char, BTreeSet<String>> {
    let mut out: BTreeMap<char, BTreeSet<String>> = BTreeMap::new();
    for (path, text) in crate::rswalk::rust_files(std::path::Path::new("crates")) {
        let name = path.replace('\\', "/");
        // 시험 안의 글자는 화면에 안 나온다 — 세면 잡음만 는다.
        if name.contains("_test") || name.contains("/tests/") {
            continue;
        }
        let mut rest = text.as_str();
        while let Some(i) = rest.find("\\u{") {
            rest = &rest[i + 3..];
            let Some(j) = rest.find('}') else { break };
            if let Ok(n) = u32::from_str_radix(&rest[..j], 16) {
                if let Some(c) = char::from_u32(n) {
                    // 제어 문자·이스케이프는 글꼴과 무관하다.
                    if !c.is_control() && c != '\u{1b}' {
                        out.entry(c).or_default().insert(name.clone());
                    }
                }
            }
            rest = &rest[j + 1..];
        }
    }
    out
}

pub(crate) fn run() -> std::process::ExitCode {
    let used = collect();
    let loaded: Vec<(&str, Vec<u8>)> = FONTS
        .iter()
        .filter_map(|(n, p)| std::fs::read(p).ok().map(|d| (*n, d)))
        .collect();
    if loaded.is_empty() {
        println!("폴백 글꼴을 하나도 읽지 못했다 — 이 PC 에서는 셀 수 없다");
        return std::process::ExitCode::SUCCESS;
    }
    let mut missing: Vec<(char, String)> = Vec::new();
    for (c, where_) in &used {
        let found = loaded.iter().any(|(_, d)| has_glyph(d, *c).unwrap_or(false));
        if !found {
            let first = where_.iter().next().cloned().unwrap_or_default();
            let more = match where_.len() {
                1 => String::new(),
                n => format!(" 외 {}곳", n - 1),
            };
            missing.push((*c, format!("{first}{more}")));
        }
    }
    println!(
        "글자 {}개 · 폴백 글꼴 {}개({}) · 어디에도 없는 글자 {}개",
        used.len(),
        loaded.len(),
        loaded.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "),
        missing.len()
    );
    for (c, wher) in &missing {
        println!("  U+{:04X} {c}  {wher}", *c as u32);
    }
    if !missing.is_empty() {
        println!(
            "\n이 글자들은 **두부(\u{25a1})로 보일 수 있다.** egui 기본 글꼴이 덮어 주는 것도\n\
             있으므로 목록을 받아 화면으로 확인할 것 — `nabi cli screenshot` 이 가장 빠르다."
        );
    }
    // 막지 않는다 — egui 기본 글꼴이 덮어 주는 것도 있어 오탐이 섞인다.
    std::process::ExitCode::SUCCESS
}
