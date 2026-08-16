//! 줄 단위 강조 조각(Span) — syntect 한 줄 실행과 조각으로부터의 LayoutJob 조립.
//!
//! 조각만 캐시해 두면 글꼴 크기가 바뀌거나 다시 그릴 때 syntect를 다시 돌리지 않아도 된다.
//! 길이는 **바이트** 기준(LayoutJob이 원문을 그대로 잘라 붙이므로 문자 수보다 바이트가 맞다).

use egui::text::LayoutJob;
use syntect::highlighting::{FontStyle, HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::{ParseState, SyntaxSet};

/// 한 줄 안에서 서식이 같은 구간: 바이트 길이 + 전경색 + 기울임.
#[derive(Clone, PartialEq, Debug)]
pub struct Span {
    pub len: u32,
    pub color: egui::Color32,
    pub italic: bool,
}

/// 한 줄의 조각들(개행 문자 포함 — 원문과 1:1 대응).
pub type LineSpans = Vec<Span>;

/// 한 줄을 강조하고 파서/강조 상태를 그 줄 **끝** 상태로 전진시킨다.
pub fn hl_line(
    line: &str,
    ps: &SyntaxSet,
    parse: &mut ParseState,
    hs: &mut HighlightState,
    hl: &Highlighter,
) -> LineSpans {
    let ops = parse.parse_line(line, ps).unwrap_or_default();
    HighlightIterator::new(hs, &ops, line, hl)
        .filter(|(_, piece)| !piece.is_empty())
        .map(|(st, piece)| Span {
            len: piece.len() as u32,
            color: egui::Color32::from_rgb(st.foreground.r, st.foreground.g, st.foreground.b),
            italic: st.font_style.contains(FontStyle::ITALIC),
        })
        .collect()
}

/// 캐시된 조각으로 LayoutJob을 만든다(syntect 재실행 없음).
///
/// 조각 길이의 합이 원문과 어긋나면 `None` — 캐시가 깨진 것이므로 호출 쪽이 전체 재계산으로
/// 되돌린다. TextEdit는 갤리 텍스트가 원문과 정확히 같아야 커서 위치가 어긋나지 않는다.
pub fn build_job(text: &str, lines: &[LineSpans], fsize: f32) -> Option<LayoutJob> {
    let font = egui::FontId::monospace(fsize);
    let mut job = LayoutJob::default();
    let mut off = 0usize;
    for spans in lines {
        for s in spans {
            let end = off + s.len as usize;
            let piece = text.get(off..end)?;
            let fmt = egui::TextFormat {
                font_id: font.clone(),
                color: s.color,
                italics: s.italic,
                ..Default::default()
            };
            job.append(piece, 0.0, fmt);
            off = end;
        }
    }
    (off == text.len()).then_some(job)
}

#[cfg(test)]
mod tests {
    use super::{build_job, LineSpans, Span};

    fn sp(len: u32) -> Span {
        Span { len, color: egui::Color32::WHITE, italic: false }
    }

    #[test]
    fn job_text_matches_source() {
        // 0.36: append가 동일 서식 인접 구간을 병합한다 — 구간 수를 보려면 색을 다르게.
        let c = |n: u8| egui::Color32::from_rgb(n, n, n);
        let text = "ab\ncd\n";
        let lines: Vec<LineSpans> = vec![
            vec![Span { len: 1, color: c(1), italic: false }, Span { len: 2, color: c(2), italic: false }],
            vec![Span { len: 3, color: c(3), italic: false }],
        ];
        let job = build_job(text, &lines, 12.0).expect("길이 합이 맞아야 함");
        assert_eq!(job.text, text, "갤리 텍스트는 원문과 정확히 같아야 커서가 안 어긋난다");
        assert_eq!(job.sections.len(), 3);
    }

    #[test]
    fn length_mismatch_is_rejected() {
        // 캐시가 깨진 상태를 그대로 쓰면 커서/선택이 통째로 어긋난다 — None으로 걸러야 한다.
        assert!(build_job("abcd", &[vec![sp(2)]], 12.0).is_none(), "합이 모자라면 거절");
        assert!(build_job("ab", &[vec![sp(9)]], 12.0).is_none(), "원문을 넘으면 거절");
    }

    #[test]
    fn empty_text_yields_empty_job() {
        let job = build_job("", &[], 12.0).expect("빈 문서도 유효");
        assert!(job.text.is_empty());
    }
}
