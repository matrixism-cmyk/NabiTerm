//! tree-sitter 정밀 구문 강조(T6-2) — 주요 언어 정적 링크, 줄 스팬으로 변환.
//!
//! ≤512KB 문서에서 편집 세대(hl_gen)마다 전체를 재파싱한다(이 크기에선 수 ms —
//! 증분 InputEdit는 후속). 결과는 기존 페인트 경로와 같은 [`LineSpans`]로 변환해
//! ropehl(syntect)과 동일 소비자를 쓴다. 지원 밖 언어/큰 문서는 호출측이 ropehl로 폴백.
//!
//! 색은 표준 캡처 이름(keyword/string/comment/…) → 고정 팔레트 매핑(다크 테마 기준).

use crate::editbuf::EditBuf;
use crate::editorhlspans::{LineSpans, Span};
use std::cell::RefCell;
use std::collections::HashMap;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// tree-sitter를 쓰는 최대 문서 크기 — 초과는 syntect 체크포인트(ropehl) 폴백.
pub const TS_CAP: usize = 512 * 1024;

/// 표준 캡처 이름(부분 접두 일치) — 인덱스가 팔레트 인덱스가 된다.
const CAPTURES: &[&str] = &[
    "keyword", "string", "comment", "function", "type", "constant", "number",
    "property", "attribute", "operator", "punctuation", "variable", "constructor",
    "label", "escape", "embedded",
];

/// 다크 크롬에 맞춘 팔레트(CAPTURES와 1:1).
const PALETTE: &[egui::Color32] = &[
    egui::Color32::from_rgb(0xc7, 0x8b, 0xdb), // keyword 보라
    egui::Color32::from_rgb(0x98, 0xc3, 0x79), // string 녹색
    egui::Color32::from_rgb(0x7f, 0x8b, 0x98), // comment 회청
    egui::Color32::from_rgb(0x61, 0xaf, 0xef), // function 하늘
    egui::Color32::from_rgb(0xe5, 0xc0, 0x7b), // type 노랑
    egui::Color32::from_rgb(0xd1, 0x9a, 0x66), // constant 주황
    egui::Color32::from_rgb(0xd1, 0x9a, 0x66), // number 주황
    egui::Color32::from_rgb(0xe0, 0x6c, 0x75), // property 적
    egui::Color32::from_rgb(0xe5, 0xc0, 0x7b), // attribute 노랑
    egui::Color32::from_rgb(0x56, 0xb6, 0xc2), // operator 시안
    egui::Color32::from_rgb(0xab, 0xb2, 0xbf), // punctuation 중립
    egui::Color32::from_rgb(0xd4, 0xdc, 0xe4), // variable 본문
    egui::Color32::from_rgb(0xe5, 0xc0, 0x7b), // constructor 노랑
    egui::Color32::from_rgb(0xe0, 0x6c, 0x75), // label 적
    egui::Color32::from_rgb(0x56, 0xb6, 0xc2), // escape 시안
    egui::Color32::from_rgb(0xd4, 0xdc, 0xe4), // embedded 본문
];

/// 확장자 → (언어, highlights 쿼리). 지원 밖이면 None.
fn config_for(ext: &str) -> Option<HighlightConfiguration> {
    let (lang, hl): (tree_sitter::Language, &str) = match ext {
        "rs" => (tree_sitter_rust::LANGUAGE.into(), tree_sitter_rust::HIGHLIGHTS_QUERY),
        "json" => (tree_sitter_json::LANGUAGE.into(), tree_sitter_json::HIGHLIGHTS_QUERY),
        "toml" => (tree_sitter_toml_ng::LANGUAGE.into(), tree_sitter_toml_ng::HIGHLIGHTS_QUERY),
        "py" => (tree_sitter_python::LANGUAGE.into(), tree_sitter_python::HIGHLIGHTS_QUERY),
        "js" | "mjs" | "cjs" => (
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
        ),
        _ => return None,
    };
    let mut cfg = HighlightConfiguration::new(lang, ext, hl, "", "").ok()?;
    cfg.configure(CAPTURES);
    Some(cfg)
}

/// 캡처 인덱스 → 색(부분 일치 매핑은 configure가 수행 — 인덱스가 곧 팔레트 인덱스).
fn color_of(idx: usize) -> egui::Color32 {
    PALETTE.get(idx).copied().unwrap_or(PALETTE[11])
}

struct DocTs {
    ext: String,
    gen_seen: u64,
    /// 줄별 스팬(전체 문서) — 세대가 같으면 재사용.
    lines: Vec<LineSpans>,
}

thread_local! {
    static CACHE: RefCell<HashMap<u64, DocTs>> = RefCell::new(HashMap::new());
    static CONFIGS: RefCell<HashMap<String, std::rc::Rc<HighlightConfiguration>>> = RefCell::new(HashMap::new());
}

/// 지원 언어인지(호출측 폴백 판단).
pub fn supported(ext: &str) -> bool {
    matches!(ext, "rs" | "json" | "toml" | "py" | "js" | "mjs" | "cjs")
}

/// 보이는 창의 줄 스팬. 미지원/초과/실패 시 None(→ ropehl 폴백).
pub fn window_spans(id: u64, eb: &EditBuf, ext: &str, first: usize, last: usize) -> Option<Vec<LineSpans>> {
    if !supported(ext) || eb.rope.len_bytes() > TS_CAP {
        return None;
    }
    let cfg = CONFIGS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(v) = c.get(ext) {
            return Some(v.clone());
        }
        let v = std::rc::Rc::new(config_for(ext)?);
        c.insert(ext.to_string(), v.clone());
        Some(v)
    })?;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 16 {
            c.clear();
        }
        let d = c.entry(id).or_insert_with(|| DocTs { ext: ext.to_string(), gen_seen: eb.hl_gen.wrapping_sub(1), lines: Vec::new() });
        if d.ext != ext {
            *d = DocTs { ext: ext.to_string(), gen_seen: eb.hl_gen.wrapping_sub(1), lines: Vec::new() };
        }
        if d.gen_seen != eb.hl_gen || d.lines.is_empty() {
            d.lines = full_highlight(&cfg, &eb.rope.to_string())?;
            d.gen_seen = eb.hl_gen;
        }
        Some((first..last).map(|l| d.lines.get(l).cloned().unwrap_or_default()).collect())
    })
}

/// 전체 텍스트를 하이라이트해 줄별 스팬으로 자른다.
fn full_highlight(cfg: &HighlightConfiguration, text: &str) -> Option<Vec<LineSpans>> {
    let mut hl = Highlighter::new();
    let events = hl.highlight(cfg, text.as_bytes(), None, |_| None).ok()?;
    let n_lines = text.split('\n').count();
    let mut lines: Vec<LineSpans> = Vec::with_capacity(n_lines);
    let (mut cur_line_spans, mut stack): (LineSpans, Vec<usize>) = (Vec::new(), Vec::new());
    let mut line_start = 0usize; // 현재 줄 시작 바이트.
    let push_span = |spans: &mut LineSpans, len: usize, cap: Option<usize>| {
        if len == 0 {
            return;
        }
        let color = cap.map(color_of).unwrap_or(PALETTE[11]);
        // 인접 동일색 병합(스팬 수 절약).
        if let Some(last) = spans.last_mut() {
            if last.color == color && !last.italic {
                last.len += len as u32;
                return;
            }
        }
        spans.push(Span { len: len as u32, color, italic: false });
    };
    for ev in events {
        match ev.ok()? {
            HighlightEvent::HighlightStart(h) => stack.push(h.0),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let cap = stack.last().copied();
                let mut at = start;
                // 조각이 줄 경계를 넘으면 줄 단위로 자른다. 개행 바이트는 그 줄의 마지막
                // 스팬에 포함한다(ropehl의 "{src}\n" 규약과 동일 — layout_spans가 소화).
                while at < end {
                    match text.as_bytes()[at..end].iter().position(|&b| b == b'\n') {
                        Some(nl) => {
                            push_span(&mut cur_line_spans, nl + 1, cap);
                            lines.push(std::mem::take(&mut cur_line_spans));
                            line_start = at + nl + 1;
                            at += nl + 1;
                        }
                        None => {
                            push_span(&mut cur_line_spans, end - at, cap);
                            at = end;
                        }
                    }
                }
            }
        }
    }
    lines.push(cur_line_spans);
    let _ = line_start;
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> EditBuf {
        EditBuf::new_buf(text, "UTF-8".into(), "LF")
    }

    /// 각 줄 스팬 길이 합 = 줄 바이트 수 + 개행(마지막 줄 제외) — layout_spans 규약.
    #[test]
    fn line_span_lengths_cover_lines() {
        let src = "fn main() {\n    let s = \"한글 문자열\"; // 주석\n}\n";
        let eb = buf(src);
        let w = window_spans(11, &eb, "rs", 0, eb.rope.len_lines()).expect("rust 지원");
        for (i, spans) in w.iter().enumerate() {
            let line = eb.line_string(i);
            let want = line.len() + if i + 1 < w.len() { 1 } else { 0 }; // +개행.
            let got: usize = spans.iter().map(|s| s.len as usize).sum();
            assert!(got == want || got == line.len(), "{i}행 길이 {got} vs {want}");
        }
    }

    /// 키워드와 문자열이 서로 다른 색을 받는다(정밀 강조의 최소 관측).
    #[test]
    fn keywords_and_strings_differ() {
        let eb = buf("fn f() { let x = \"s\"; }\n");
        let w = window_spans(12, &eb, "rs", 0, 1).expect("rust 지원");
        let colors: std::collections::HashSet<_> =
            w[0].iter().map(|s| (s.color.r(), s.color.g(), s.color.b())).collect();
        assert!(colors.len() >= 3, "키워드/문자열/본문 최소 3색: {colors:?}");
    }

    /// 미지원 확장자·상한 초과는 None(→ syntect 폴백).
    #[test]
    fn fallback_conditions() {
        let eb = buf("hello\n");
        assert!(window_spans(13, &eb, "zig", 0, 1).is_none(), "미지원 언어");
        assert!(!supported("zig") && supported("rs"));
    }
}
