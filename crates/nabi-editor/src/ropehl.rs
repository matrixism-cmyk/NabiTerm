//! rope 뷰포트 구문 강조(T6-1) — 보이는 창만 계산하는 체크포인트 하이라이터.
//!
//! IncHl(editorhlinc)은 전체 `&str`을 매번 해시해 rope 대용량과 맞지 않는다. 여기서는
//! **가장 가까운 체크포인트(100줄 간격)에서 재개해 보이는 창까지만** syntect를 돌리고,
//! 편집 신호(EditBuf.hl_gen/hl_dirty_from)로 그 줄 이후 체크포인트·스팬을 버린다.
//! 그래서 비용이 문서 크기가 아니라 "체크포인트 간격 + 보이는 줄 수"에 묶인다.
//! (첫 진입 시 깊은 줄로 점프하면 0줄부터 상태를 쌓는다 — 그 줄까지 1회 비용.)

use crate::editbuf::EditBuf;
use crate::editorhlspans::{hl_line, LineSpans};
use crate::editorsyntax::{assets, current_theme, mapped_syntax};
use std::cell::RefCell;
use std::collections::HashMap;
use syntect::highlighting::{HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack};

/// 체크포인트 간격(줄) — syntect branch point 만료(128줄)보다 짧게(IncHl과 동일 근거).
const CKPT: usize = 100;
/// 강조를 켜는 최대 문서 크기 — 이보다 크면 평문(과도한 1회 워밍업 방지). 8MB.
pub const ROPE_HL_CAP: usize = 8 * 1024 * 1024;
/// 스팬 캐시 상한(줄) — 초과 시 통째로 비운다(스크롤로 무한히 자라지 않게).
const MAX_SPANS: usize = 4096;
/// 문서 캐시 상한(탭 수).
const MAX_DOCS: usize = 16;

type State = (ParseState, HighlightState);

struct DocHl {
    ext: String,
    theme: String,
    seen_gen: u64,
    /// (줄 번호, 그 줄 직전 상태) — 오름차순.
    ckpt: Vec<(usize, State)>,
    spans: HashMap<usize, LineSpans>,
}

thread_local! {
    static CACHE: RefCell<HashMap<u64, DocHl>> = RefCell::new(HashMap::new());
}

/// 보이는 줄 범위의 스팬을 계산해 `Vec<Option<LineSpans>>`(first..last 순)로 돌려준다.
/// 문법을 못 찾거나 문서가 상한을 넘으면 None(호출측 평문 폴백).
pub fn window_spans(id: u64, eb: &mut EditBuf, ext_raw: &str, first: usize, last: usize) -> Option<Vec<LineSpans>> {
    if eb.rope.len_bytes() > ROPE_HL_CAP {
        return None;
    }
    let ext = ext_raw.to_string();
    let theme = current_theme();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > MAX_DOCS {
            c.clear();
        }
        let d = c.entry(id).or_insert_with(|| DocHl {
            ext: ext.clone(), theme: theme.clone(), seen_gen: eb.hl_gen.wrapping_sub(1),
            ckpt: Vec::new(), spans: HashMap::new(),
        });
        // 언어/테마 변경 = 전면 무효.
        if d.ext != ext || d.theme != theme {
            *d = DocHl { ext: ext.clone(), theme: theme.clone(), seen_gen: eb.hl_gen, ckpt: Vec::new(), spans: HashMap::new() };
        }
        // 편집 신호 소비: 변경 줄부터 체크포인트·스팬 폐기.
        if d.seen_gen != eb.hl_gen {
            let from = eb.hl_dirty_from;
            d.ckpt.retain(|(l, _)| *l <= from);
            d.spans.retain(|l, _| *l < from);
            d.seen_gen = eb.hl_gen;
            eb.hl_dirty_from = eb.rope.len_lines(); // 소비 완료 — 다음 편집(min)이 다시 낮춘다.
        }
        if d.spans.len() > MAX_SPANS {
            d.spans.clear();
        }
        compute(d, eb, first, last)
    })
}

/// first..last 스팬을 보장하고 모아 돌려준다(내부: 가장 가까운 체크포인트에서 전진).
fn compute(d: &mut DocHl, eb: &EditBuf, first: usize, last: usize) -> Option<Vec<LineSpans>> {
    let a = assets().read().ok()?;
    let syn = mapped_syntax(&d.ext)
        .and_then(|n| a.ps.find_syntax_by_name(&n))
        .or_else(|| a.ps.find_syntax_by_extension(&d.ext))?;
    let th = a.themes.themes.get(&d.theme).or_else(|| a.themes.themes.values().next())?;
    let hl = Highlighter::new(th);
    // 재개 지점: last 미만 중 가장 큰 체크포인트(없으면 문서 시작).
    let missing_from = (first..last).find(|l| !d.spans.contains_key(l)).unwrap_or(last);
    if missing_from < last {
        let (mut line, mut st) = d
            .ckpt
            .iter()
            .rev()
            .find(|(l, _)| *l <= missing_from)
            .map(|(l, s)| (*l, s.clone()))
            .unwrap_or_else(|| {
                (0, (ParseState::new(syn), HighlightState::new(&hl, ScopeStack::new())))
            });
        let total = eb.rope.len_lines();
        while line < last.min(total) {
            // 체크포인트 적립(그 줄 "직전" 상태).
            if line.is_multiple_of(CKPT) && d.ckpt.last().map(|(l, _)| *l < line).unwrap_or(true) {
                d.ckpt.push((line, st.clone()));
            }
            let src = eb.line_string(line);
            let with_nl = format!("{src}\n"); // syntect는 개행 포함 줄 전제.
            let spans = hl_line(&with_nl, &a.ps, &mut st.0, &mut st.1, &hl);
            if line >= first {
                d.spans.insert(line, spans);
            } else if line >= missing_from.saturating_sub(CKPT) {
                // 창 바로 앞 구간은 다음 스크롤 업 대비로 저장해 둔다.
                d.spans.insert(line, spans);
            }
            line += 1;
        }
    }
    Some((first..last).map(|l| d.spans.get(&l).cloned().unwrap_or_default()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "LF")
    }

    /// 체크포인트 경로가 처음부터 순차 계산한 결과와 같아야 한다(정확성).
    #[test]
    fn window_matches_sequential() {
        crate::editorsyntax::init(std::path::Path::new("."), String::new(), Default::default());
        let src: String = (0..300).map(|i| format!("let x{i} = {i}; // c{i}\n")).collect();
        let mut eb = buf(&src);
        // 창 하나를 깊은 곳에서 요청(체크포인트 재개 경로).
        let deep = window_spans(1, &mut eb, "rs", 250, 260).expect("rust 문법");
        // 같은 줄을 순차로 계산한 결과와 대조.
        let mut eb2 = buf(&src);
        let seq = window_spans(2, &mut eb2, "rs", 0, 260).expect("rust 문법");
        assert_eq!(deep, seq[250..260].to_vec(), "체크포인트 재개가 순차 결과와 달라선 안 된다");
    }

    /// 편집하면 그 줄부터 무효화되고, 다시 요청하면 새 내용 기준으로 나온다.
    #[test]
    fn edit_invalidates_from_line() {
        crate::editorsyntax::init(std::path::Path::new("."), String::new(), Default::default());
        let src: String = (0..120).map(|i| format!("// line {i}\n")).collect();
        let mut eb = buf(&src);
        let before = window_spans(3, &mut eb, "rs", 100, 110).expect("문법");
        // 50번째 줄 시작에 주석 아닌 코드를 삽입 — 이후 줄의 재계산 강제.
        eb.set_cursor(eb.rope.line_to_char(50));
        eb.insert("let y = \"");
        let after = window_spans(3, &mut eb, "rs", 100, 110).expect("문법");
        // 문자열이 열린 채라 이후 줄 색이 달라진다(무효화가 실제로 일어났는지의 관측).
        assert_ne!(before, after, "편집 이후 줄 강조가 갱신되어야 한다");
    }
}
