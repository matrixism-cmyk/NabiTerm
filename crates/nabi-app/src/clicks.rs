//! 더블/트리플 클릭 선택(단어·URL·줄). selection.rs에서 분리.
#![allow(clippy::too_many_arguments)]

use crate::selection::{extract_selection, Sel};
use nabi_types::PaneId;
use nabi_vt::{RenderCell, TermModel, Theme};

/// 더블/트리플 클릭을 처리한다(트리플=줄 전체, 더블=단어/URL). 없으면 None.
pub(crate) fn handle_click_select(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pane: PaneId,
    model: &TermModel,
    theme: &Theme,
    selection: &mut Option<Sel>,
) -> Option<String> {
    handle_triple_click(ui, rect, ch, pane, model, theme, selection)
        .or_else(|| handle_double_click(ui, rect, cw, ch, pane, model, theme, selection))
}

/// col 위치의 단어 경계(셀 인덱스, 포함)를 찾는다. 단어 문자가 아니면 None.
pub(crate) fn word_bounds(row: &[RenderCell], col: usize) -> Option<(usize, usize)> {
    let is_word = |t: &str| {
        !t.is_empty()
            && t.chars()
                .all(|ch| ch.is_alphanumeric() || "_-./~:@".contains(ch))
    };
    if col >= row.len() || !is_word(&row[col].text) {
        return None;
    }
    let mut s = col;
    while s > 0 && is_word(&row[s - 1].text) {
        s -= 1;
    }
    let mut e = col;
    while e + 1 < row.len() && is_word(&row[e + 1].text) {
        e += 1;
    }
    Some((s, e))
}

/// 더블클릭한 (행 r, render 셀 열 c)의 단어(또는 URL 전체)를 선택하고 추출 텍스트를 돌려준다.
/// c는 이미 render 인덱스(와이드 보정 완료)여야 한다.
pub(crate) fn double_click_word(
    pane: PaneId,
    rows: &[Vec<RenderCell>],
    r: usize,
    c: usize,
    selection: &mut Option<Sel>,
) -> Option<String> {
    let row = rows.get(r)?;
    // URL 위 더블클릭이면 단어가 아니라 URL 전체를 선택한다.
    if let Some(sp) = nabi_render::row_urls(row)
        .into_iter()
        .find(|s| c >= s.start && c <= s.end)
    {
        *selection = Some(Sel {
            pane,
            ar: r,
            ac: sp.start,
            hr: r,
            hc: sp.end,
            rect: false,
        });
        let text = extract_selection(rows, r, sp.start, r, sp.end, &[], false);
        return (!text.is_empty()).then_some(text);
    }
    let (s, e) = word_bounds(row, c)?;
    *selection = Some(Sel {
        pane,
        ar: r,
        ac: s,
        hr: r,
        hc: e,
        rect: false,
    });
    let text = extract_selection(rows, r, s, r, e, &[], false);
    (!text.is_empty()).then_some(text)
}

fn handle_double_click(
    ui: &egui::Ui,
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pane: PaneId,
    model: &TermModel,
    theme: &Theme,
    selection: &mut Option<Sel>,
) -> Option<String> {
    let pos = ui.input(|i| {
        i.pointer
            .button_double_clicked(egui::PointerButton::Primary)
            .then(|| i.pointer.interact_pos())
            .flatten()
    })?;
    if !rect.contains(pos) {
        return None;
    }
    // 줄바꿈된 링크(URL/경로) 위 더블클릭이면 한 줄만이 아니라 링크 전체를 선택한다.
    if let Some(text) = double_click_screen_url(rect, cw, ch, pane, model, theme, pos, selection) {
        return Some(text);
    }
    let r = ((pos.y - rect.top()) / ch).floor().max(0.0) as usize;
    let vcol = ((pos.x - rect.left()) / cw).floor().max(0.0) as usize;
    let c = model.render_index_at(r as u16, vcol); // 시각열→render 인덱스(와이드 보정).
    let rows = model.render_rows(theme);
    double_click_word(pane, &rows, r, c, selection)
}

/// 더블클릭 위치가 (줄바꿈 포함) 링크 위면 그 링크 전체를 선택하고 텍스트를 돌려준다.
fn double_click_screen_url(
    rect: egui::Rect,
    cw: f32,
    ch: f32,
    pane: PaneId,
    model: &TermModel,
    theme: &Theme,
    pos: egui::Pos2,
    selection: &mut Option<Sel>,
) -> Option<String> {
    let su = crate::paneurl::screen_url_at(model, theme, rect, cw, ch, pos)?;
    let (sr, sc, _) = *su.segs.first()?;
    let (er, _, ec) = *su.segs.last()?;
    *selection = Some(Sel { pane, ar: sr, ac: sc, hr: er, hc: ec, rect: false });
    let rows = model.render_rows(theme);
    let wrapped: Vec<bool> = (0..rows.len()).map(|r| model.row_wrapped(r as u16)).collect();
    let text = extract_selection(&rows, sr, sc, er, ec, &wrapped, false);
    (!text.is_empty()).then_some(text)
}

fn handle_triple_click(
    ui: &egui::Ui,
    rect: egui::Rect,
    ch: f32,
    pane: PaneId,
    model: &TermModel,
    theme: &Theme,
    selection: &mut Option<Sel>,
) -> Option<String> {
    let pos = ui.input(|i| {
        i.pointer
            .button_triple_clicked(egui::PointerButton::Primary)
            .then(|| i.pointer.interact_pos())
            .flatten()
    })?;
    if !rect.contains(pos) {
        return None;
    }
    let r = ((pos.y - rect.top()) / ch).floor().max(0.0) as usize;
    let rows = model.render_rows(theme);
    rows.get(r)?; // 행 범위 밖이면 무시.
    // 소프트랩된 논리 줄 전체를 선택한다(여러 시각 행에 걸쳐도 한 줄로 복사됨).
    let wrapped: Vec<bool> = (0..rows.len()).map(|i| model.row_wrapped(i as u16)).collect();
    let (sr, er) = wrapped_line_bounds(&wrapped, r);
    let last = rows[er].iter().rposition(|c| !c.text.trim().is_empty()).unwrap_or(0);
    *selection = Some(Sel { pane, ar: sr, ac: 0, hr: er, hc: last, rect: false });
    let text = extract_selection(&rows, sr, 0, er, last, &wrapped, false);
    (!text.is_empty()).then_some(text)
}

/// 소프트랩을 고려한 논리 줄의 시작·끝 시각 행. `wrapped[i]`=행 i가 i+1로 이어짐.
pub(crate) fn wrapped_line_bounds(wrapped: &[bool], r: usize) -> (usize, usize) {
    let mut start = r;
    while start > 0 && *wrapped.get(start - 1).unwrap_or(&false) {
        start -= 1;
    }
    let mut end = r;
    while end + 1 < wrapped.len() && wrapped[end] {
        end += 1;
    }
    (start, end)
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
    fn word_bounds_includes_path_chars() {
        let row = cells("foo bar.baz qux");
        assert_eq!(word_bounds(&row, 5), Some((4, 10))); // "bar.baz"
        assert_eq!(word_bounds(&row, 3), None); // 공백
    }

    #[test]
    fn word_bounds_includes_user_at_host() {
        let row = cells("ssh root@example.com");
        assert_eq!(word_bounds(&row, 4), Some((4, 19))); // "root@example.com"
    }

    #[test]
    fn wrapped_line_bounds_spans_softwrap() {
        // wrapped[i]=행 i가 다음으로 이어짐. 행1을 클릭하면 논리줄 0..=2.
        let w = [true, true, false, false];
        assert_eq!(super::wrapped_line_bounds(&w, 1), (0, 2));
        assert_eq!(super::wrapped_line_bounds(&w, 0), (0, 2));
        assert_eq!(super::wrapped_line_bounds(&w, 3), (3, 3)); // 단독 줄.
        assert_eq!(super::wrapped_line_bounds(&[true], 0), (0, 0)); // 끝에서 넘침 방지.
    }

    #[test]
    fn double_click_selects_full_url() {
        let rows = vec![cells("x https://a.bc/y z")];
        let mut sel = None;
        // 행 0, render 셀 5('p') 위 더블클릭 → URL 전체 선택.
        let t = double_click_word(nabi_types::next_pane_id(), &rows, 0, 5, &mut sel);
        assert_eq!(t.as_deref(), Some("https://a.bc/y"));
    }
}
