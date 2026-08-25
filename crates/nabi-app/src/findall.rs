//! **모든 창에서 찾기** — 열린 pane 전부의 스크롤백을 한 번에 훑는다.
//!
//! pane 하나 안 검색(`find.rs`)은 있었지만 "그 오류를 어느 창에서 봤더라"에는 답하지
//! 못했다. 창을 하나씩 눌러 가며 Ctrl+F를 다시 치는 것이 지금까지의 방법이었다.
//!
//! 검색 규칙(리터럴·정규식·단어 단위·스마트케이스)은 `find.rs`의 매처를 **그대로 쓴다**.
//! 같은 질문에 두 곳이 다르게 답하면 사용자는 어느 쪽을 믿어야 할지 모른다.
//!
//! ## 멈추지 않게 하는 선
//!
//! 창 20개 × 스크롤백 10만 줄이면 200만 줄이다. 전부 모으면 UI가 멈춘다. 그래서 pane별
//! 결과 수와 전체 결과 수에 상한을 두고, **넘으면 넘었다고 화면에 적는다** — 조용히 자르면
//! 사용자는 없는 것으로 오해한다.

use crate::find::Matcher;
use nabi_types::PaneId;

/// pane 하나에서 가져올 최대 결과.
pub(crate) const PER_PANE: usize = 200;
/// 전체 최대 결과.
pub(crate) const TOTAL: usize = 1000;

/// 찾은 줄 하나.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub pane: PaneId,
    pub title: String,
    /// 절대 줄 번호(0 = 스크롤백 맨 위) — 눌렀을 때 그 자리로 보낸다.
    pub abs_line: usize,
    pub text: String,
}

/// 한 pane의 결과 묶음.
#[derive(Clone, Debug)]
pub(crate) struct PaneHits {
    pub hits: Vec<Hit>,
    /// 상한에 걸려 더 있는데 못 담았는가.
    pub more: bool,
}

/// 한 pane의 줄들에서 일치하는 것을 모은다. `from_abs`는 `lines[0]`의 절대 줄 번호.
///
/// 결과 텍스트는 앞뒤 공백을 떼고 길면 자른다 — 목록으로 훑어보는 것이 목적이라
/// 긴 줄 하나가 창을 가로로 늘리면 안 된다.
pub(crate) fn scan_pane(
    pane: PaneId,
    title: &str,
    lines: &[String],
    from_abs: usize,
    m: &Matcher,
    budget: usize,
) -> PaneHits {
    let cap = PER_PANE.min(budget);
    let mut hits = Vec::new();
    let mut more = false;
    for (i, line) in lines.iter().enumerate() {
        if !m.is_match(line) {
            continue;
        }
        if hits.len() >= cap {
            more = true;
            break;
        }
        hits.push(Hit {
            pane,
            title: title.to_string(),
            abs_line: from_abs + i,
            text: clip(line.trim(), 160),
        });
    }
    PaneHits { hits, more }
}

/// 긴 줄은 잘라 준다(목록 표시용).
fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}\u{2026}", &s[..i]),
        None => s.to_string(),
    }
}

/// 검색어 이력에 한 줄 올린다 — 최신 우선·중복 제거·상한.
///
/// 같은 것을 다시 치게 만들지 않는 것이 이력의 전부다. 중복을 지우지 않으면 목록이
/// 같은 단어로 금세 가득 찬다.
pub(crate) fn push_history(hist: &mut Vec<String>, q: &str) {
    let q = q.trim();
    if q.is_empty() {
        return;
    }
    hist.retain(|h| h != q);
    hist.insert(0, q.to_string());
    hist.truncate(20);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::find::build_matcher;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn m(q: &str) -> Matcher {
        build_matcher(q, false, false).unwrap()
    }

    #[test]
    fn it_finds_matching_lines_and_remembers_where_they_were() {
        let src = lines(&["hello", "an error here", "bye", "another error"]);
        let got = scan_pane(PaneId(1), "shell", &src, 100, &m("error"), TOTAL);
        assert_eq!(got.hits.len(), 2);
        assert_eq!(got.hits[0].abs_line, 101, "절대 줄 번호는 from_abs 를 더한 값이다");
        assert_eq!(got.hits[1].abs_line, 103);
        assert_eq!(got.hits[0].text, "an error here");
        assert!(!got.more);
    }

    /// 상한을 넘으면 **넘었다고 알린다** — 조용히 자르면 없는 것으로 오해한다.
    #[test]
    fn hitting_the_cap_is_reported_not_hidden() {
        let src: Vec<String> = (0..500).map(|i| format!("error {i}")).collect();
        let got = scan_pane(PaneId(1), "t", &src, 0, &m("error"), TOTAL);
        assert_eq!(got.hits.len(), PER_PANE);
        assert!(got.more, "더 있다는 사실이 남아야 한다");
    }

    /// 전체 예산이 적으면 그만큼만 가져온다(창이 여럿일 때 앞쪽이 다 먹지 않게).
    #[test]
    fn a_small_budget_limits_one_pane() {
        let src: Vec<String> = (0..50).map(|i| format!("x {i}")).collect();
        let got = scan_pane(PaneId(1), "t", &src, 0, &m("x"), 5);
        assert_eq!(got.hits.len(), 5);
        assert!(got.more);
    }

    #[test]
    fn long_lines_are_clipped_for_the_list() {
        let long = "e".repeat(400);
        let got = scan_pane(PaneId(1), "t", &lines(&[&long]), 0, &m("eee"), TOTAL);
        assert!(got.hits[0].text.chars().count() <= 161);
        assert!(got.hits[0].text.ends_with('\u{2026}'));
    }

    /// 표시용으로 앞뒤 공백을 뗀다 — 들여쓰기 깊은 줄이 목록에서 밀려 보이지 않게.
    #[test]
    fn surrounding_space_is_trimmed_for_display() {
        let got = scan_pane(PaneId(1), "t", &lines(&["      spaced out      "]), 0, &m("spaced"), TOTAL);
        assert_eq!(got.hits[0].text, "spaced out");
    }

    #[test]
    fn the_search_history_keeps_the_newest_first_without_duplicates() {
        let mut h = Vec::new();
        push_history(&mut h, "a");
        push_history(&mut h, "b");
        push_history(&mut h, "a");
        assert_eq!(h, ["a", "b"]);
        push_history(&mut h, "   ");
        assert_eq!(h.len(), 2, "빈 검색어는 담지 않는다");
        for i in 0..30 {
            push_history(&mut h, &format!("q{i}"));
        }
        assert_eq!(h.len(), 20, "이력이 무한정 늘면 안 된다");
        assert_eq!(h[0], "q29");
    }

    /// 정규식·단어 단위도 pane 하나 검색과 **같은 규칙**이어야 한다.
    #[test]
    fn it_uses_the_same_rules_as_the_single_pane_search() {
        let src = lines(&["cat", "concat", "the cat sat"]);
        let whole = build_matcher("cat", false, true).unwrap();
        let got = scan_pane(PaneId(1), "t", &src, 0, &whole, TOTAL);
        assert_eq!(got.hits.len(), 2, "concat 은 단어 단위로 걸리지 않는다");
        let re = build_matcher(r"c.t$", true, false).unwrap();
        let got = scan_pane(PaneId(1), "t", &src, 0, &re, TOTAL);
        assert_eq!(got.hits.len(), 2);
    }
}
