//! 텍스트 덤프 API — 백로그 저장·제어 평면 capture/tail이 쓰는 추출 계열.
//! (렌더 셀 추출은 render.rs, 여기는 평문/SGR 문자열.)

use crate::cell::Theme;
use crate::grid::TermModel;
use crate::render::{to_rgba, UNDERLINES};
use alacritty_terminal::index::Column;
use alacritty_terminal::term::cell::Flags;
use nabi_types::Rgba;

impl TermModel {
    /// 히스토리+화면 전체에서 마지막 max_lines줄의 평문 텍스트(세션 백로그 저장용).
    pub fn dump_text(&self, max_lines: usize) -> String {
        let total = self.total_abs_lines();
        let mut lines = self.lines_abs_text(0, total);
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop(); // 꼬리 빈 줄 제거.
        }
        let skip = lines.len().saturating_sub(max_lines);
        lines[skip..].join("\r\n")
    }

    /// 현재 보이는 화면(스크롤백 제외) 상위 max_rows줄의 평문 — 인코딩 감지 등
    /// 가벼운 샘플용(히스토리 전체를 만드는 dump_text와 달리 화면 행만 읽어 저비용).
    pub fn visible_text(&self, max_rows: usize) -> String {
        let rows = (self.size().rows() as usize).min(max_rows);
        (0..rows)
            .map(|r| self.visible_row_text(r as u16))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 완료된 절대 줄 수 마커(히스토리 + 커서 화면 행). tail 델타 스트림이
    /// 이 값의 증가분만큼 신규 줄을 읽는다(CP-5 G2).
    pub fn line_marker(&self) -> usize {
        self.history_size() + self.cursor_line().max(0) as usize
    }

    /// 히스토리+화면의 절대 줄 총수(범위 캡처의 끝 기본값).
    pub fn total_abs_lines(&self) -> usize {
        self.history_size() + self.size().rows() as usize
    }

    /// 절대 줄 구간 [from, to)의 평문 텍스트(0 = 히스토리 맨 위). 범위는 클램프.
    pub fn lines_abs_text(&self, from: usize, to: usize) -> Vec<String> {
        let hist = self.history_size() as i32;
        let rows = self.size().rows() as i32;
        let cols = self.size().cols() as usize;
        let grid = self.term.grid();
        let mut out = Vec::new();
        for a in from..to {
            let l = a as i32 - hist;
            if !(-hist..rows).contains(&l) {
                continue;
            }
            let row = &grid[alacritty_terminal::index::Line(l)];
            let mut s = String::with_capacity(cols);
            for c in 0..cols {
                let cell = &row[Column(c)];
                if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    s.push(cell.c);
                }
            }
            out.push(s.trim_end().to_string());
        }
        out
    }

    /// 절대 줄 구간 [from, to)를 SGR(24비트 색·굵게·밑줄) 보존 텍스트로 덤프한다
    /// (capture --escapes — tmux `-e` 패리티, CP-8).
    pub fn lines_abs_sgr(&self, from: usize, to: usize, theme: &Theme) -> Vec<String> {
        let hist = self.history_size() as i32;
        let rows = self.size().rows() as i32;
        let cols = self.size().cols() as usize;
        let grid = self.term.grid();
        let mut out = Vec::new();
        for a in from..to {
            let l = a as i32 - hist;
            if !(-hist..rows).contains(&l) {
                continue;
            }
            let row = &grid[alacritty_terminal::index::Line(l)];
            let mut s = String::with_capacity(cols * 2);
            let mut cur: Option<(Rgba, Rgba, bool, bool)> = None;
            for c in 0..cols {
                let cell = &row[Column(c)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let fg = to_rgba(cell.fg, theme.fg, theme);
                let bg = to_rgba(cell.bg, theme.bg, theme);
                let bold = cell.flags.contains(Flags::BOLD);
                let ul = cell.flags.intersects(UNDERLINES);
                let want = (fg, bg, bold, ul);
                if cur != Some(want) {
                    cur = Some(want);
                    s.push_str(&format!(
                        "\x1b[0{}{};38;2;{};{};{};48;2;{};{};{}m",
                        if bold { ";1" } else { "" },
                        if ul { ";4" } else { "" },
                        fg.r, fg.g, fg.b, bg.r, bg.g, bg.b,
                    ));
                }
                s.push(cell.c);
            }
            s.push_str("\x1b[0m");
            out.push(s);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::grid::TermModel;
    use nabi_types::GridSize;

    #[test]
    fn sgr_dump_preserves_color() {
        let mut m = TermModel::new(GridSize::new(10, 3), 10);
        m.process(b"\x1b[31mR\x1b[0m");
        let theme = crate::cell::Theme::default();
        let lines = m.lines_abs_sgr(0, m.total_abs_lines(), &theme);
        // 빨강(ANSI 1) 전경 SGR이 보존된다.
        assert!(lines[0].contains(";38;2;"), "{}", lines[0]);
        assert!(lines[0].contains('R'));
    }

    #[test]
    fn range_dump_clamps() {
        let mut m = TermModel::new(GridSize::new(10, 2), 10);
        m.process(b"a\r\nb\r\nc");
        let all = m.lines_abs_text(0, 999);
        assert!(all.iter().any(|l| l == "a"));
        assert!(all.iter().any(|l| l == "c"));
    }

    #[test]
    fn visible_text_caps_rows() {
        let mut m = TermModel::new(GridSize::new(20, 4), 100);
        m.process(b"alpha\r\nbeta\r\ngamma");
        assert!(m.visible_text(2).lines().count() <= 2); // 최대 행수 제한(텔레그램 폴백 캡처).
        let full = m.visible_text(10); // 화면 행수(4)로 클램프.
        assert!(full.contains("alpha") && full.contains("beta") && full.contains("gamma"));
    }
}
