//! 목록 파일에서 **읽히는 항목만 살린다**(배치 AA) — 브라우저 탭·SFTP 탭 공용.
//!
//! 두 복원 경로가 똑같이 이랬다:
//!
//! ```text
//! let Ok(saves) = ron::from_str::<Vec<T>>(&text) else { return out };   // 하나 깨지면 전부 포기
//! ```
//!
//! 탭 열두 개를 열어 두고 껐는데 그중 하나가 깨져 있으면 **열두 개를 전부** 잃는다.
//! 항목 경계가 분명한 목록에서 그럴 이유가 없다.
//!
//! 세션 저장소(`nabi_session::salvage`)와 설정(`nabi_config`)에도 같은 손질을 했다.
//! 셋 다 **부분 실패를 전체 손실로 처리하던** 같은 결함이다.

use serde::de::DeserializeOwned;

/// RON 목록을 읽되, 걸리는 항목만 버린다. 돌려주는 것은 `(살린 것, 버린 수)`.
///
/// 목록 문법 자체가 깨졌으면 항목 경계를 알 수 없으므로 빈 목록이다 — 짐작해서 건지면
/// 엉뚱한 것이 나온다.
pub(crate) fn parse_vec<T: DeserializeOwned>(text: &str) -> (Vec<T>, usize) {
    // 멀쩡한 파일이 대부분이다. 그 길을 먼저 가고, 실패했을 때만 하나씩 뜯는다.
    if let Ok(v) = ron::from_str::<Vec<T>>(text) {
        return (v, 0);
    }
    let Ok(items) = ron::from_str::<Vec<ron::Value>>(text) else {
        return (Vec::new(), 0);
    };
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for it in items {
        match it.into_rust::<T>() {
            Ok(v) => kept.push(v),
            Err(_) => dropped += 1,
        }
    }
    (kept, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// 브라우저 탭이 실제로 저장되는 모양 — `(경로, 보기, 정렬, 내림차순, 숨김표시)`.
    ///
    /// 이름을 붙이는 이유는 시험을 읽기 쉽게 하려는 것만이 아니다. 다섯 칸짜리 익명 튜플은
    /// 무엇이 무엇인지 세어 봐야 알 수 있어서, 순서를 바꾸는 실수가 조용히 지나간다.
    type BrowserTab = (String, u8, u8, bool, bool);

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Tab {
        path: String,
        view: u8,
    }

    #[test]
    fn a_healthy_list_parses_whole() {
        let text = r#"[(path:"a",view:1),(path:"b",view:2)]"#;
        let (got, dropped): (Vec<Tab>, _) = parse_vec(text);
        assert_eq!(got.len(), 2);
        assert_eq!(dropped, 0);
    }

    /// **이 시험이 이 파일의 이유다.** 항목 하나가 깨져도 나머지는 열린다.
    #[test]
    fn one_broken_entry_does_not_close_every_tab() {
        // 가운데 항목의 view 가 문자열이다(손으로 고치다 흔히 나는 꼴).
        let text = r#"[(path:"a",view:1),(path:"b",view:"둘"),(path:"c",view:3)]"#;
        let (got, dropped): (Vec<Tab>, _) = parse_vec(text);
        assert_eq!(dropped, 1, "깨진 항목 하나만 버린다");
        let paths: Vec<&str> = got.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, vec!["a", "c"], "나머지 탭은 그대로 열려야 한다");
    }

    #[test]
    fn broken_syntax_yields_nothing() {
        let (got, dropped): (Vec<Tab>, _) = parse_vec("[(path:\"a\"");
        assert!(got.is_empty());
        assert_eq!(dropped, 0, "항목 경계를 모르니 버린 것도 셀 수 없다");
    }

    #[test]
    fn an_empty_list_is_empty() {
        let (got, dropped): (Vec<Tab>, _) = parse_vec("[]");
        assert!(got.is_empty());
        assert_eq!(dropped, 0);
    }

    #[test]
    fn tuple_shaped_entries_work_too() {
        // 브라우저 탭은 튜플로 저장된다 — 구조체와 같은 길을 지나야 한다.
        let text = r#"[("a",1,0,false,true),("b",9,9,true,false)]"#;
        let (got, dropped): (Vec<BrowserTab>, _) = parse_vec(text);
        assert_eq!(dropped, 0);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "a");
    }

    #[test]
    fn a_broken_tuple_entry_is_dropped_alone() {
        let text = r#"[("a",1,0,false,true),("b","x",0,false,true),("c",2,0,false,true)]"#;
        let (got, dropped): (Vec<BrowserTab>, _) = parse_vec(text);
        assert_eq!(dropped, 1);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].0, "c");
    }
}
