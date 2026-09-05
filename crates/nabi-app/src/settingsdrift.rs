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

#[cfg(test)]
mod tests {
    use super::*;

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
