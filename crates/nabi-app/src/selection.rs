//! 마우스 텍스트 선택 상태 + 선택 텍스트 추출(드래그→릴리스 시 자동 복사).
//!
//! 더블/트리플 클릭(단어·URL·줄) 선택은 [`crate::clicks`]에 있다.
#![allow(clippy::needless_range_loop, clippy::type_complexity)]

use nabi_types::PaneId;
use nabi_vt::RenderCell;

/// 한 pane의 선택 영역(앵커→헤드, 셀 좌표).
#[derive(Clone, Copy)]
pub(crate) struct Sel {
    pub pane: PaneId,
    pub ar: usize,
    pub ac: usize,
    pub hr: usize,
    pub hc: usize,
    /// 사각(블록) 선택 여부(Alt+드래그).
    pub rect: bool,
}

impl Sel {
    /// 앵커/헤드를 정렬한 (start_row, start_col, end_row, end_col).
    pub(crate) fn norm(&self) -> (usize, usize, usize, usize) {
        let a = (self.ar, self.ac);
        let h = (self.hr, self.hc);
        let (s, e) = if a <= h { (a, h) } else { (h, a) };
        (s.0, s.1, e.0, e.1)
    }

    /// 모드별 (sr,sc,er,ec): 사각이면 (위,왼,아래,오른), 줄흐름이면 norm().
    pub(crate) fn span(&self) -> (usize, usize, usize, usize) {
        if self.rect {
            (
                self.ar.min(self.hr),
                self.ac.min(self.hc),
                self.ar.max(self.hr),
                self.ac.max(self.hc),
            )
        } else {
            self.norm()
        }
    }

    /// 앵커와 헤드가 같은(빈) 선택인지.
    pub(crate) fn is_empty(&self) -> bool {
        self.ar == self.hr && self.ac == self.hc
    }
}

/// 포인터 드래그로 선택을 갱신하고 (정규화 범위, 릴리스 여부)를 돌려준다.
pub(crate) fn track_selection(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pane: PaneId,
    model: &nabi_vt::TermModel,
    selection: &mut Option<Sel>,
) -> (Option<(usize, usize, usize, usize)>, bool, bool) {
    let (pressed, down, released, ppos, shift, alt) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.interact_pos(),
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });
    // 드래그를 **시작**할 때만 가림을 따진다. 이미 이 pane에서 끌고 있는 선택은 포인터가
    // 잠깐 다른 레이어 위를 지나도 이어져야 한다(창 가장자리를 스치며 긋는 경우).
    let dragging_here = down && selection.as_ref().is_some_and(|s| s.pane == pane);
    let on_pane = |p: &egui::Pos2| match dragging_here {
        true => rect.contains(*p),
        false => crate::paneio::pointer_on_pane(ui, rect, *p),
    };
    if let Some(p) = ppos.filter(on_pane) {
        // 시각 열 → render 셀 인덱스(와이드 문자 보정). 선택/렌더/추출 모두 render 인덱스로 일치.
        let (r, vcol) = crate::panecell::at(rect, cw, ch, p);
        let c = model.render_index_at(r as u16, vcol);
        if pressed {
            // Shift+클릭: 기존 선택의 헤드를 클릭 지점으로 확장. 아니면 새 선택.
            if shift && selection.as_ref().is_some_and(|s| s.pane == pane) {
                if let Some(s) = selection.as_mut() {
                    s.hr = r;
                    s.hc = c;
                }
            } else {
                *selection = Some(Sel {
                    pane,
                    ar: r,
                    ac: c,
                    hr: r,
                    hc: c,
                    rect: alt,
                });
            }
        } else if down {
            if let Some(s) = selection.as_mut() {
                if s.pane == pane {
                    s.hr = r;
                    s.hc = c;
                }
            }
        }
    }
    match (*selection).filter(|s| s.pane == pane && !s.is_empty()) {
        Some(s) => (Some(s.span()), released, s.rect),
        None => (None, false, false),
    }
}

/// 선택 범위의 텍스트를 추출한다. `wrapped[r]`가 참인 줄(소프트랩)은 trailing 공백·개행
/// 없이 다음 줄과 이어 붙인다(빈 슬라이스면 모두 일반 줄로 취급 — 줄마다 trim+개행).
///
/// `rect`가 참이면 사각(블록) 선택: 모든 줄에서 [sc..ec] 열만 잘라 줄마다 개행한다.
pub(crate) fn extract_selection(
    rows: &[Vec<RenderCell>],
    sr: usize,
    sc: usize,
    er: usize,
    ec: usize,
    wrapped: &[bool],
    rect: bool,
) -> String {
    let last = er.min(rows.len().saturating_sub(1));
    let mut out = String::new();
    for r in sr..=last {
        let row = &rows[r];
        let lastc = row.len().saturating_sub(1);
        let (start, end) = if rect {
            (sc.min(lastc), ec.min(lastc))
        } else {
            let start = if r == sr { sc } else { 0 };
            let end = if r == er { ec.min(lastc) } else { lastc };
            (start, end)
        };
        let mut line = String::new();
        for c in start..=end {
            let t = &row[c].text;
            line.push_str(if t.is_empty() { " " } else { t });
        }
        // 줄흐름 + 소프트랩이면 그대로 이어 붙이고, 아니면 trim + 개행.
        if !rect && r != last && wrapped.get(r).copied().unwrap_or(false) {
            out.push_str(&line);
        } else {
            out.push_str(line.trim_end());
            if r != last {
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_types::{CellAttrs, Rgba};

    fn cells(s: &str) -> Vec<RenderCell> {
        s.chars()
            .map(|c| RenderCell {
                text: c.to_string(),
                fg: Rgba::WHITE,
                bg: Rgba::BLACK,
                attrs: CellAttrs::default(),
                ul_color: None,
            })
            .collect()
    }

    #[test]
    fn extracts_single_line_range() {
        let rows = vec![cells("hello world")];
        assert_eq!(extract_selection(&rows, 0, 0, 0, 4, &[], false), "hello");
        assert_eq!(extract_selection(&rows, 0, 6, 0, 10, &[], false), "world");
    }

    #[test]
    fn extracts_multi_line_with_newline() {
        let rows = vec![cells("abc"), cells("def")];
        assert_eq!(extract_selection(&rows, 0, 1, 1, 1, &[], false), "bc\nde");
    }

    #[test]
    fn extracts_rectangular_block() {
        // 사각 선택: 두 줄 모두 1..2열만 잘라 줄마다 개행.
        let rows = vec![cells("abcd"), cells("efgh")];
        assert_eq!(extract_selection(&rows, 0, 1, 1, 2, &[], true), "bc\nfg");
    }

    #[test]
    fn span_line_vs_rect() {
        // 앵커(0,5)→헤드(2,1): 줄흐름은 행우선 norm, 사각은 코너 min/max.
        let mk = |rect| Sel {
            pane: nabi_types::PaneId::new(1),
            ar: 0,
            ac: 5,
            hr: 2,
            hc: 1,
            rect,
        };
        assert_eq!(mk(false).span(), (0, 5, 2, 1));
        assert_eq!(mk(true).span(), (0, 1, 2, 5));
    }
}
