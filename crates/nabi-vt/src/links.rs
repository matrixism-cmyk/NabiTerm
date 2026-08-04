//! OSC 8 명시적 하이퍼링크 — alacritty가 셀에 저장한 (id,uri)를 렌더/클릭용으로 노출한다.
//!
//! 휴리스틱 URL 감지(nabi-render urls/screenurls)와 별개로, 프로그램이 `ESC]8;;URI ST`로
//! 명시한 링크를 그대로 클릭/밑줄 처리한다(예: `ls --hyperlink`, eza, gcc).

use crate::grid::TermModel;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::Flags;

impl TermModel {
    /// 한 보이는 행의 셀별 OSC 8 (id, uri) — render_row와 같은 셀 정렬(WIDE 스페이서 제외).
    fn row_hyperlinks(&self, row: u16) -> Vec<Option<(String, String)>> {
        let cols = self.size().cols() as usize;
        if row >= self.size().rows() {
            return Vec::new();
        }
        let line = self.visible_line(row);
        let gr = &self.term.grid()[line];
        let mut v = Vec::with_capacity(cols);
        for c in 0..cols {
            let cell = &gr[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            v.push(cell.hyperlink().map(|h| (h.id().to_string(), h.uri().to_string())));
        }
        v
    }

    /// 화면 전체에서 명시적 하이퍼링크가 있는 셀 표시(밑줄용) — 행별 bool 맵(render 셀 정렬).
    pub fn hyperlink_map(&self) -> Vec<Vec<bool>> {
        let cols = self.size().cols() as usize;
        (0..self.size().rows())
            .map(|r| {
                let line = self.visible_line(r);
                let gr = &self.term.grid()[line];
                let mut v = Vec::with_capacity(cols);
                for c in 0..cols {
                    let cell = &gr[Column(c)];
                    if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        continue;
                    }
                    v.push(cell.hyperlink().is_some());
                }
                v
            })
            .collect()
    }

    /// (row,col) 셀이 명시적 하이퍼링크면 같은 링크(동일 id)가 이어진 (시작열,끝열,uri)를 돌려준다.
    /// col은 render 셀 인덱스(클릭 위치에서 산출). 다른 셀에 없으면 None.
    pub fn hyperlink_span_at(&self, row: u16, col: usize) -> Option<(usize, usize, String)> {
        let links = self.row_hyperlinks(row);
        let (id, uri) = links.get(col)?.clone()?;
        let mut s = col;
        while s > 0 && links[s - 1].as_ref().is_some_and(|h| h.0 == id) {
            s -= 1;
        }
        let mut e = col;
        while e + 1 < links.len() && links[e + 1].as_ref().is_some_and(|h| h.0 == id) {
            e += 1;
        }
        Some((s, e, uri))
    }
}

#[cfg(test)]
mod tests {
    use crate::TermModel;
    use nabi_types::GridSize;

    #[test]
    fn parses_osc8_hyperlink_span() {
        let mut m = TermModel::new(GridSize::new(40, 5), 100);
        // OSC 8로 "link" 4글자에 URL을 입힌 뒤 닫는다.
        m.process(b"x \x1b]8;;https://ex.com\x07link\x1b]8;;\x07 y");
        // 'l'은 인덱스 2(앞 "x ").
        let span = m.hyperlink_span_at(0, 2).expect("링크 셀이어야 함");
        assert_eq!(span.2, "https://ex.com");
        assert_eq!((span.0, span.1), (2, 5)); // "link" 4셀.
        // 링크 밖(인덱스 0)은 None.
        assert!(m.hyperlink_span_at(0, 0).is_none());
        // 맵에도 4개만 표시.
        let map = m.hyperlink_map();
        assert_eq!(map[0].iter().filter(|b| **b).count(), 4);
    }
}
