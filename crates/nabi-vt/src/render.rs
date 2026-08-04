//! 그리드 → RenderCell 추출 + alacritty 색 변환(렌더러가 코어에 직접 의존하지 않도록).

use crate::cell::{idx_to_rgba, RenderCell, Theme};
use crate::grid::TermModel;
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use nabi_types::{ansi16, CellAttrs, Rgba};

/// alacritty 색 → 구체 RGBA(이름색은 테마/ANSI16, 인덱스는 256 팔레트).
pub(crate) fn to_rgba(c: Color, default: Rgba, theme: &Theme) -> Rgba {
    match c {
        Color::Spec(rgb) => Rgba::rgb(rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => idx_to_rgba(i),
        Color::Named(n) => match n {
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
                theme.fg
            }
            NamedColor::Background => theme.bg,
            NamedColor::Cursor => default,
            other => {
                let i = other as usize;
                if i < 16 {
                    ansi16(i as u8) // Black..BrightWhite는 0..=15 순서.
                } else if (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize)
                    .contains(&i)
                {
                    ansi16((i - NamedColor::DimBlack as usize) as u8)
                } else {
                    default
                }
            }
        },
    }
}

/// 밑줄 계열 플래그(단일/이중/물결/점선/파선) 묶음.
pub(crate) const UNDERLINES: Flags = Flags::UNDERLINE
    .union(Flags::DOUBLE_UNDERLINE)
    .union(Flags::UNDERCURL)
    .union(Flags::DOTTED_UNDERLINE)
    .union(Flags::DASHED_UNDERLINE);

impl TermModel {
    /// 화면 전체(스크롤백 오프셋 반영)를 행별 RenderCell로 추출한다.
    pub fn render_rows(&self, theme: &Theme) -> Vec<Vec<RenderCell>> {
        let rows = self.size().rows() as usize;
        (0..rows).map(|r| self.render_row(r, theme)).collect()
    }

    /// 보이는 한 행만 RenderCell로 추출한다(URL 호버/클릭 단일 행 조회용 — 전체 할당 회피).
    pub fn render_row(&self, r: usize, theme: &Theme) -> Vec<RenderCell> {
        let (rows, cols) = (self.size().rows() as usize, self.size().cols() as usize);
        if r >= rows {
            return Vec::new();
        }
        let row = &self.term.grid()[self.visible_line(r as u16)];
        let mut v = Vec::with_capacity(cols);
        for c in 0..cols {
            let cell = &row[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue; // 와이드 글자의 자리표시 셀은 건너뜀(렌더러 규약).
            }
            let mut attrs = CellAttrs::default();
            attrs.set(CellAttrs::BOLD, cell.flags.contains(Flags::BOLD));
            attrs.set(CellAttrs::ITALIC, cell.flags.contains(Flags::ITALIC));
            attrs.set(CellAttrs::UNDERLINE, cell.flags.contains(Flags::UNDERLINE));
            attrs.set(CellAttrs::DOUBLE_UNDERLINE, cell.flags.contains(Flags::DOUBLE_UNDERLINE));
            attrs.set(CellAttrs::UNDERCURL, cell.flags.contains(Flags::UNDERCURL));
            attrs.set(CellAttrs::DOTTED_UNDERLINE, cell.flags.contains(Flags::DOTTED_UNDERLINE));
            attrs.set(CellAttrs::DASHED_UNDERLINE, cell.flags.contains(Flags::DASHED_UNDERLINE));
            attrs.set(CellAttrs::INVERSE, cell.flags.contains(Flags::INVERSE));
            attrs.set(CellAttrs::WIDE, cell.flags.contains(Flags::WIDE_CHAR));
            let mut fg = to_rgba(cell.fg, theme.fg, theme);
            let mut bg = to_rgba(cell.bg, theme.bg, theme);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            // SGR 58 밑줄 색(없으면 None → 렌더러가 전경색 사용).
            let ul_color = cell.underline_color().map(|c| to_rgba(c, fg, theme));
            let text = if cell.c == ' ' && cell.zerowidth().is_none() {
                String::new() // 빈 셀 규약(렌더러가 그리기 생략).
            } else {
                let mut t = cell.c.to_string();
                if let Some(zw) = cell.zerowidth() {
                    t.extend(zw.iter());
                }
                t
            };
            v.push(RenderCell { text, fg, bg, attrs, ul_color });
        }
        v
    }

    /// 클릭의 '시각 열'(픽셀/cw로 센 열)을 render 셀 인덱스로 변환한다. WIDE(CJK 등)
    /// 문자는 2열을 차지하므로, 텍스트 왼쪽에 와이드 글자가 있어도 선택/클릭 위치가 어긋나지
    /// 않는다(render_row와 같은 셀 정렬: WIDE 스페이서 제외).
    pub fn render_index_at(&self, row: u16, visual_col: usize) -> usize {
        if row >= self.size().rows() {
            return visual_col;
        }
        let cols = self.size().cols() as usize;
        let gr = &self.term.grid()[self.visible_line(row)];
        let (mut vcol, mut idx) = (0usize, 0usize);
        for c in 0..cols {
            let cell = &gr[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let w = if cell.flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            if visual_col < vcol + w {
                return idx;
            }
            vcol += w;
            idx += 1;
        }
        idx
    }

    /// 보이는 row번째 줄의 평문 텍스트(검색용 — 빈 셀은 공백).
    pub(crate) fn visible_row_text(&self, row: u16) -> String {
        let cols = self.size().cols() as usize;
        let grid = self.term.grid();
        let line = self.visible_line(row);
        let r = &grid[line];
        let mut s = String::with_capacity(cols);
        for c in 0..cols {
            let cell = &r[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            s.push(cell.c);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_map_to_rgba() {
        let th = Theme::default();
        assert_eq!(to_rgba(Color::Named(NamedColor::Red), th.fg, &th), ansi16(1));
        assert_eq!(to_rgba(Color::Named(NamedColor::BrightBlue), th.fg, &th), ansi16(12));
        assert_eq!(to_rgba(Color::Indexed(196), th.fg, &th), idx_to_rgba(196));
        assert_eq!(to_rgba(Color::Named(NamedColor::Foreground), th.fg, &th), th.fg);
    }

    #[test]
    fn sgr_colors_render() {
        let mut m = TermModel::new(nabi_types::GridSize::new(10, 3), 10);
        m.process(b"\x1b[31mR\x1b[0m");
        let rows = m.render_rows(&Theme::default());
        assert_eq!(rows[0][0].text, "R");
        assert_eq!(rows[0][0].fg, ansi16(1)); // 빨강.
    }

    #[test]
    fn parses_styled_underlines_and_color() {
        use nabi_types::CellAttrs;
        let mut m = TermModel::new(nabi_types::GridSize::new(10, 3), 10);
        // 4:3=undercurl + 58:2::255:0:0=빨강 밑줄색, 4:2=이중, 리셋.
        m.process(b"\x1b[4:3;58:2::255:0:0mA\x1b[m\x1b[4:2mB");
        let rows = m.render_rows(&Theme::default());
        assert!(rows[0][0].attrs.contains(CellAttrs::UNDERCURL), "A=undercurl");
        assert_eq!(rows[0][0].ul_color, Some(nabi_types::Rgba::rgb(255, 0, 0)));
        assert!(rows[0][1].attrs.contains(CellAttrs::DOUBLE_UNDERLINE), "B=double");
        assert!(rows[0][1].ul_color.is_none(), "B 밑줄색 리셋됨");
    }

    #[test]
    fn render_index_at_accounts_for_wide_chars() {
        // 가·나는 와이드(각 2시각열), a·b·c는 1열. render 셀=[가,나,a,b,c].
        let mut m = TermModel::new(nabi_types::GridSize::new(20, 3), 10);
        m.process("가나abc".as_bytes());
        assert_eq!(m.render_index_at(0, 0), 0); // 가
        assert_eq!(m.render_index_at(0, 1), 0); // 가의 둘째 시각열도 같은 셀
        assert_eq!(m.render_index_at(0, 2), 1); // 나
        assert_eq!(m.render_index_at(0, 4), 2); // a
        assert_eq!(m.render_index_at(0, 6), 4); // c
    }
}
