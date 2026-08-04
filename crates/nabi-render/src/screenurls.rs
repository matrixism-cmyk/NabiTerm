//! 화면 전체의 소프트랩-인지 링크 감지 — 줄바꿈된 URL/경로도 한 링크로 이어 잡는다.
#![allow(clippy::needless_range_loop)]

use crate::urls::row_urls;
use nabi_vt::RenderCell;

/// 화면의 링크 한 개 — 소프트랩으로 여러 시각 행에 걸칠 수 있어 행별 세그먼트로 보관.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenUrl {
    /// (행, 시작열, 끝열)[포함] 세그먼트들. 줄바꿈되면 2개 이상.
    pub segs: Vec<(usize, usize, usize)>,
    pub url: String,
}

impl ScreenUrl {
    /// (row, col)이 이 링크의 한 세그먼트에 포함되는가.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        self.segs.iter().any(|&(r, s, e)| r == row && col >= s && col <= e)
    }
}

/// 화면 전체에서 링크를 찾는다 — `wrapped[i]`가 true면 행 i가 다음 행으로 소프트랩된 것이라
/// 한 논리 줄로 이어 붙여 감지하므로, 줄바꿈된 URL/경로도 끊기지 않는다.
pub fn screen_urls(rows: &[Vec<RenderCell>], wrapped: &[bool]) -> Vec<ScreenUrl> {
    let mut out = Vec::new();
    let mut r = 0;
    while r < rows.len() {
        let mut end = r;
        while *wrapped.get(end).unwrap_or(&false) && end + 1 < rows.len() {
            end += 1; // 소프트랩 그룹 확장.
        }
        // URL 후보(':'·'.')가 있는 그룹만 처리(없으면 스캔·복제 생략 — 성능).
        let hint = (r..=end).any(|gr| rows[gr].iter().any(|c| c.text.contains([':', '.'])));
        if hint {
            let (mut cells, mut map) = (Vec::new(), Vec::new());
            for gr in r..=end {
                for (c, cell) in rows[gr].iter().enumerate() {
                    cells.push(cell.clone());
                    map.push((gr, c));
                }
            }
            for sp in row_urls(&cells) {
                let mut segs: Vec<(usize, usize, usize)> = Vec::new();
                for k in sp.start..=sp.end.min(map.len().saturating_sub(1)) {
                    let (gr, gc) = map[k];
                    match segs.last_mut() {
                        Some(s) if s.0 == gr && s.2 + 1 == gc => s.2 = gc,
                        _ => segs.push((gr, gc, gc)),
                    }
                }
                if !segs.is_empty() {
                    out.push(ScreenUrl { segs, url: sp.url });
                }
            }
        }
        r = end + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::screen_urls;
    use nabi_types::{CellAttrs, Rgba};
    use nabi_vt::RenderCell;

    fn cells(s: &str) -> Vec<RenderCell> {
        s.chars()
            .map(|c| RenderCell { text: c.to_string(), fg: Rgba::WHITE, bg: Rgba::BLACK, attrs: CellAttrs::default(), ul_color: None })
            .collect()
    }

    #[test]
    fn screen_url_spans_softwrapped_rows() {
        // 경로가 줄바꿈(소프트랩)되면 두 행을 이어 붙여 한 링크로 잡는다.
        let urls = screen_urls(&[cells("C:/Users/Use"), cells("r/file.txt x")], &[true, false]);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "C:/Users/User/file.txt");
        assert!(urls[0].segs.iter().any(|&(r, _, _)| r == 0)); // 첫 행 세그먼트.
        assert!(urls[0].segs.iter().any(|&(r, _, _)| r == 1)); // 둘째 행 세그먼트.
    }

    // 실제 터미널 파이프라인 전체 검증: TermModel에 긴 URL을 흘려보내 소프트랩시키고,
    // render_rows + row_wrapped로 만든 입력에 screen_urls를 돌려 한 링크로 이어지는지 본다.
    // (사용자 신고: "줄바꿈된 링크 밑줄이 전혀 이어지지 않음" — 런타임 경로 회귀 가드.)
    #[test]
    fn end_to_end_softwrapped_url_is_one_link() {
        use nabi_types::GridSize;
        use nabi_vt::{TermModel, Theme};
        let mut m = TermModel::new(GridSize::new(20, 6), 100);
        // 20열을 넘기는 긴 URL → 자동 줄바꿈(WRAPLINE).
        m.process(b"https://example.com/very/long/path/page.html");
        let theme = Theme::default();
        let rows = m.render_rows(&theme);
        let wrapped: Vec<bool> = (0..rows.len()).map(|r| m.row_wrapped(r as u16)).collect();
        assert!(wrapped[0], "첫 행이 소프트랩이어야 함");
        let urls = screen_urls(&rows, &wrapped);
        assert_eq!(urls.len(), 1, "줄바꿈돼도 링크는 하나여야 함: {urls:?}");
        assert_eq!(urls[0].url, "https://example.com/very/long/path/page.html");
        // 두 행 이상에 세그먼트가 걸쳐야 한다(밑줄이 이어짐).
        let row_count = urls[0].segs.iter().map(|&(r, _, _)| r).collect::<std::collections::BTreeSet<_>>().len();
        assert!(row_count >= 2, "링크가 여러 행에 걸쳐야 함: {:?}", urls[0].segs);
    }
}
