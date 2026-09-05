//! 설정 화면이 **다시 어긋나지 않게** 붙잡는 시험.
//!
//! ## 왜 시험으로 붙잡나
//!
//! 긴 설명을 표의 칸 안에 넣으면 그 칸이 설명 길이만큼 넓어진다. 칸 폭은 그 칸에 들어간
//! 것 중 가장 넓은 것이 정하기 때문이다. 그러면 설정 창 전체가 늘어난다.
//!
//! 2026-09-05 에 이것을 고쳤는데, **한 페이지만 고치고 다른 페이지를 빠뜨렸다.** 사용자가
//! 짚어 준 것이 바로 그 빠뜨린 페이지였다("원격 연결 안의 '열 더 보기'"). 손으로 훑어
//! 고치면 다음에도 같은 일이 생긴다.
//!
//! 그래서 훑는 일을 시험에게 맡긴다. 새로 넣는 사람도 여기서 걸린다.
//!
//! ## 무엇을 보나
//!
//! 표 한 줄이 이렇게 생기면 잡는다 — **이름 칸 다음이 곧바로 또 글이고 줄이 끝나는** 꼴.
//!
//! ```text
//! ui.label(tr(lang, "settings.extracol"));
//! ui.label(tr(lang, "settings.extracolhint"));   ← 이 줄이 오른쪽 칸을 넓힌다
//! ui.end_row();
//! ```
//!
//! 설명을 넣고 싶으면 `settingsui::help_line` 으로 표 **밖에** 적는다.
//!
//! 글자 폭을 재지 않고 모양만 본다 — 재려면 글꼴이 있어야 하고, 시험은 화면 없이 돈다.
//!
//! ## 모양만으로는 새는 자리가 있었다 (2026-09-05, 세 번째 보고)
//!
//! 위 모양 검사는 `ui.label` 두 줄만 본다. 그런데 자동화 페이지의 오너 안내는
//! `ui.label(""); ui.weak(tr(...))` 였다 — **빈 이름 칸 + weak 설명**. 모양이 달라서
//! 그냥 지나갔고, 사용자가 세 번째로 같은 것을 짚어 줬다.
//!
//! 그래서 자를 하나 더 댄다. **문구가 길면 잡는다.** 모양이 어떻든, 칸에 바로 놓인 글이
//! 길면 그 칸이 넓어지기 때문이다. 길이는 i18n 목록에서 한국어 문구를 읽어 글자로 센다.

/// 훑을 설정 화면 소스들.
///
/// 새 설정 파일을 만들면 여기 한 줄을 더한다. 빠뜨리면 그 파일만 검사에서 새는데,
/// 아래 `모든_설정_파일을_본다` 가 그것도 잡는다.
const SOURCES: &[(&str, &str)] = &[
    ("settingsui.rs", include_str!("settingsui.rs")),
    ("settingsui2.rs", include_str!("settingsui2.rs")),
    ("settingsxfer.rs", include_str!("settingsxfer.rs")),
    ("settingsa11y.rs", include_str!("settingsa11y.rs")),
    ("settingsshell.rs", include_str!("settingsshell.rs")),
    ("settingslog.rs", include_str!("settingslog.rs")),
    ("settingsprev.rs", include_str!("settingsprev.rs")),
    ("settingslsp.rs", include_str!("settingslsp.rs")),
    ("settingslists.rs", include_str!("settingslists.rs")),
    ("settingstelegram.rs", include_str!("settingstelegram.rs")),
    ("settingsfont.rs", include_str!("settingsfont.rs")),
];

/// `ui.label(tr(lang, "…"));` 한 줄인가 — 그렇다면 그 열쇠말.
fn plain_label(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t.strip_prefix("ui.label(tr(lang, \"")?;
    let (key, tail) = rest.split_once('"')?;
    (tail == "));").then_some(key)
}

/// 표 안에서 **설명이 칸을 차지한** 자리를 찾는다. (파일, 줄, 이름칸, 설명칸)
pub(crate) fn description_in_cell() -> Vec<(&'static str, usize, &'static str, &'static str)> {
    let mut out = Vec::new();
    for (name, src) in SOURCES {
        let lines: Vec<&str> = src.lines().collect();
        for i in 0..lines.len().saturating_sub(2) {
            let (Some(a), Some(b)) = (plain_label(lines[i]), plain_label(lines[i + 1])) else {
                continue;
            };
            if lines[i + 2].trim() == "ui.end_row();" {
                out.push((*name, i + 1, a, b));
            }
        }
    }
    out
}

/// 이 글자 수를 넘는 문구가 칸에 바로 놓이면 잡는다.
///
/// 설정 창은 840점 폭이고 이름 칸이 200점이니 남는 칸이 600점쯤이다. 한글 한 글자가
/// 대략 14점이라 40글자면 이미 560점 — 그 언저리부터 밀려 나기 시작한다.
#[cfg(test)]
const MAX_IN_CELL: usize = 40;

#[cfg(test)]
mod tests {
    use super::*;

    /// 여는 괄호 뒤부터 따옴표 문자열을 차례로 뽑는다(**줄바꿈을 넘어간다**).
    ///
    /// i18n 항목은 한 줄짜리와 여러 줄짜리 두 가지 모양이 있다. 한 줄만 읽으면 긴 문구를
    /// 통째로 놓친다 — 처음 만든 판이 실제로 그렇게 놓쳤다(정작 찾으려던 tg.ownerhint 를).
    fn strings_after(text: &str, from: usize) -> Vec<String> {
        let b = text.as_bytes();
        let (mut out, mut i) = (Vec::new(), from);
        while out.len() < 3 && i < b.len() {
            let Some(q) = text[i..].find('"').map(|k| i + k) else { break };
            if text[i..q].contains(')') {
                break; // 문자열이 나오기 전에 항목이 닫혔다.
            }
            let mut k = q + 1;
            let mut buf = String::new();
            while k < b.len() {
                match b[k] {
                    b'\\' => {
                        buf.push('\\');
                        k += 2;
                    }
                    b'"' => break,
                    _ => {
                        let ch = text[k..].chars().next().unwrap_or(' ');
                        buf.push(ch);
                        k += ch.len_utf8();
                    }
                }
            }
            out.push(buf);
            i = k + 1;
        }
        out
    }

    /// 키 → 한국어 문구.
    fn korean_strings() -> std::collections::HashMap<String, String> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nabi-i18n/src");
        let mut map = std::collections::HashMap::new();
        let Ok(rd) = std::fs::read_dir(&dir) else { return map };
        for e in rd.flatten() {
            if e.path().extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let Ok(t) = std::fs::read_to_string(e.path()) else { continue };
            let mut at = 0usize;
            while let Some(k) = t[at..].find('(') {
                let start = at + k;
                at = start + 1;
                let parts = strings_after(&t, start);
                if parts.len() < 3 {
                    continue;
                }
                let key = &parts[0];
                let looks_like_key = !key.is_empty()
                    && key.contains('.')
                    && key
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_');
                if looks_like_key {
                    map.insert(key.clone(), parts[2].clone());
                }
            }
        }
        map
    }

    /// 이 줄이 글을 **칸에 바로** 놓는가.
    fn puts_text_in_cell(line: &str) -> bool {
        if line.contains("help_line") || line.contains("on_hover_text") {
            return false; // 이미 표 밖이거나 툴팁이다.
        }
        ["ui.label(", "ui.weak(", "ui.colored_label(", "ui.small("]
            .iter()
            .any(|p| line.contains(p))
    }

    /// 그 줄이 쓰는 i18n 키들.
    fn keys_in(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = 0usize;
        while let Some(k) = line[at..].find("tr(lang, \"") {
            let s = at + k + "tr(lang, \"".len();
            let Some(e) = line[s..].find('"') else { break };
            out.push(line[s..s + e].to_string());
            at = s + e;
        }
        out
    }

    /// **긴 문구가 칸에 바로 놓였으면 실패한다** — 모양 검사가 놓치는 자리를 메운다.
    ///
    /// 고치는 법: 그 줄을 `settingsui::help_line` 으로 바꾼다. 표 안이라면 표를 그 자리에서
    /// 끊고 안내를 표 밖에 적는다(`settingsui2::behavior_rows` 가 본보기).
    #[test]
    fn 긴_문구를_칸에_바로_놓지_않는다() {
        let ko = korean_strings();
        // 아무것도 못 읽고 조용히 통과하는 것을 막는다.
        assert!(ko.len() > 1000, "i18n 문구를 제대로 읽지 못했다({}개)", ko.len());
        let mut bad = Vec::new();
        for (name, src) in SOURCES {
            for (n, line) in src.lines().enumerate() {
                if !puts_text_in_cell(line) {
                    continue;
                }
                for key in keys_in(line) {
                    let len = ko.get(&key).map(|s| s.chars().count()).unwrap_or(0);
                    if len > MAX_IN_CELL {
                        bad.push(format!("{name}:{} {key} ({len}자)", n + 1));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "긴 문구가 표 칸에 바로 놓였다(오른쪽이 창 밖으로 밀린다). help_line 으로 옮길 것:\n  {}",
            bad.join("\n  ")
        );
    }

    /// 위 검사의 판정이 **실제로 작동하는지** 본다.
    #[test]
    fn 길이_검사가_진짜로_잡는지_본다() {
        assert!(puts_text_in_cell("    ui.label(tr(lang, \"a.b\"));"));
        assert!(puts_text_in_cell("    ui.weak(tr(lang, \"a.b\"));"));
        assert!(!puts_text_in_cell("    settingsui::help_line(ui, tr(lang, \"a.b\"));"));
        assert!(!puts_text_in_cell("    ui.label(x).on_hover_text(tr(lang, \"a.b\"));"));
        assert_eq!(keys_in("ui.label(tr(lang, \"one.two\"))"), vec!["one.two".to_string()]);
        // 여러 줄짜리 i18n 항목도 읽어야 한다 — 이것을 못 읽어 처음에 놓쳤다.
        let ko = korean_strings();
        let owner = ko.get("tg.ownerhint").map(|s| s.chars().count()).unwrap_or(0);
        assert!(owner > MAX_IN_CELL, "여러 줄 i18n 항목을 못 읽고 있다({owner}자)");
    }

    /// 설명을 표 칸에 넣으면 창이 늘어난다 — 하나도 없어야 한다.
    ///
    /// 걸렸다면 그 설명을 `settingsui::help_line` 으로 표 밖에 옮길 것.
    /// 시험이 진짜인지 보려면 아무 설정 파일에 그 세 줄을 넣어 볼 것 — 빨개진다.
    #[test]
    fn 설명을_표_칸에_넣지_않는다() {
        let bad = description_in_cell();
        assert!(
            bad.is_empty(),
            "설명이 표 칸을 차지하고 있다(창이 넓어진다). help_line 으로 옮길 것:\n{bad:#?}"
        );
    }

    /// 위 검사가 **실제로 모든 설정 파일**을 보는지 확인한다.
    ///
    /// 목록에서 빠진 파일은 조용히 검사 밖에 있게 된다 — 그게 가장 나쁘다.
    #[test]
    fn 모든_설정_파일을_본다() {
        let listed: Vec<&str> = SOURCES.iter().map(|(n, _)| *n).collect();
        // main.rs 가 아는 설정 모듈 이름을 뽑아 대조한다.
        let main = include_str!("main.rs");
        let mut missing = Vec::new();
        for tok in main.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if tok.starts_with("settings") && tok.len() > 8 {
                let f = format!("{tok}.rs");
                // 검색·검사·드리프트 자신은 화면이 아니다.
                if matches!(tok, "settingsearch" | "settingsearchui" | "settingscan" | "settingsdrift")
                {
                    continue;
                }
                if !listed.contains(&f.as_str()) && !missing.contains(&f) {
                    missing.push(f);
                }
            }
        }
        assert!(missing.is_empty(), "검사 목록에 없는 설정 파일: {missing:?}");
    }
}
