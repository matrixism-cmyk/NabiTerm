//! 설정 화면 소스를 훑어 **실제로 그려지는 항목**을 뽑는다 — 검색 표의 드리프트 감시자.
//!
//! 사람이 관리하는 목록은 반드시 어긋난다. 그래서 목록을 믿지 않고 매번 소스에서 다시
//! 뽑아 대조한다(`settingsearch`의 시험이 이것을 쓴다). 시험 전용이라 배포본에는 안 들어간다.
//!
//! 어느 함수가 어느 페이지에 그려지는지는 `settingsui::page`의 디스패치가 사실의 출처다.
//! 그 대응만 여기 적어 두고, 항목은 전부 소스에서 읽는다.

/// 행 그리기 함수 → 그 함수가 그려지는 페이지 번호(`settingsui::page` 참조).
const FN_PAGE: &[(&str, usize)] = &[
    ("behavior_rows", 0),
    ("approvals_ui", 0),
    ("font_rows", 1),
    ("color_rows", 1),
    ("cursor_rows", 1),
    ("import_section", 1),
    ("terminal_rows", 2),
    ("tip_rows", 2),
    ("ssh_rows", 3),
    ("transfer_rows", 3),
    ("sftp_rows", 3),
    ("highlight_rows", 4),
    ("alert_rows", 4),
    ("link_rule_rows", 4),
    ("snippet_rows", 4),
    ("a11y_rows", 6),
    ("contrast_note", 6),   // a11y_rows가 부르는 하위 행(대비 경고).
];

/// 훑을 파일들.
const FILES: &[&str] = &[
    "settingsui.rs",
    "settingsui2.rs",
    "settingslists.rs",
    "themeimport.rs",
    "settingsfont.rs",
    "settingsa11y.rs",
];

/// 단위 접미사 등 "항목이 아닌" 키.
const NOT_AN_ITEM: &[&str] = &["settings.lines"];

/// 소스 폴더에서 (항목 키, 페이지)를 뽑는다.
pub(crate) fn scan(dir: &std::path::Path) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for f in FILES {
        let Ok(src) = std::fs::read_to_string(dir.join(f)) else { continue };
        for (name, page) in FN_PAGE {
            let Some(body) = function_body(&src, name) else { continue };
            for key in keys_in(body) {
                if out.iter().any(|(k, _)| *k == key) {
                    continue;
                }
                out.push((key, *page));
            }
        }
    }
    out.sort();
    out
}

/// `fn <name>(`부터 다음 함수 정의 직전까지. 없으면 None.
fn function_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("fn {name}(");
    let start = src.find(&needle)?;
    let rest = &src[start + needle.len()..];
    // 다음 함수 정의를 찾아 거기서 끊는다. 없으면 파일 끝까지.
    let end = rest.find("\nfn ").into_iter().chain(rest.find("\npub(crate) fn ")).min();
    Some(match end {
        Some(e) => &rest[..e],
        None => rest,
    })
}

/// 본문에서 설정 라벨 번역 호출의 키를 뽑는다(구역 제목·도움말·비항목 제외).
/// (여기에 그 호출 모양을 그대로 적으면 i18n 검사기가 진짜 키로 오해한다.)
fn keys_in(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    // 찾을 낱말을 조각으로 적는다. 한 덩어리로 두면 **i18n 키 검사기가 이 문자열을
    // 진짜 키로 오해한다**(실제로 걸렸다 — 있지도 않은 `settings.` 키가 없다고 실패했다).
    let needle = concat!("tr(lang, ", "\"", "settings.");
    while let Some(i) = rest.find(needle) {
        let after = &rest[i + needle.len() - "settings.".len()..];
        let Some(q) = after.find('"') else { break };
        let key = &after[..q];
        let skip = key.starts_with("settings.sec.")
            || key.ends_with("hint")
            || key.ends_with("help")
            || NOT_AN_ITEM.contains(&key);
        if !skip && !out.iter().any(|k| k == key) {
            out.push(key.to_string());
        }
        rest = &after[q..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 함수 본문을 잘못 자르면 남의 항목을 자기 페이지로 끌어온다.
    #[test]
    fn a_body_stops_at_the_next_function() {
        let src = "pub(crate) fn a_rows(x: u8) {\n  tr(lang, \"settings.one\");\n}\nfn b_rows(y: u8) {\n  tr(lang, \"settings.two\");\n}\n";
        let a = function_body(src, "a_rows").unwrap();
        assert!(a.contains("settings.one"));
        assert!(!a.contains("settings.two"), "다음 함수까지 삼켰다");
    }

    #[test]
    fn a_missing_function_is_not_an_error() {
        assert!(function_body("fn other() {}", "nope").is_none());
    }

    /// 구역 제목과 도움말은 항목이 아니다 — 검색 결과에 섞이면 눌러도 갈 곳이 없다.
    #[test]
    fn headings_and_help_text_are_not_items() {
        let body = "tr(lang, \"settings.sec.font\") tr(lang, \"settings.fontsize\") tr(lang, \"settings.fontsizehint\") tr(lang, \"settings.lines\")";
        assert_eq!(keys_in(body), vec!["settings.fontsize".to_string()]);
    }

    #[test]
    fn repeated_keys_are_listed_once() {
        let body = "tr(lang, \"settings.x\") tr(lang, \"settings.x\")";
        assert_eq!(keys_in(body).len(), 1);
    }

    /// 실제 소스에서 넉넉히 뽑혀야 한다 — 0이면 훑기가 조용히 망가진 것이다.
    #[test]
    fn the_real_sources_yield_many_items() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let got = scan(&dir);
        assert!(got.len() > 40, "너무 적다: {}", got.len());
        assert!(got.iter().all(|(k, _)| k.starts_with("settings.")));
    }
}
