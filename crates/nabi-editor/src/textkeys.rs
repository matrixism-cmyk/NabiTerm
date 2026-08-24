//! 용량 무제한 편집기의 키보드·클립보드 입력 — egui 이벤트를 [`TextBuf`] 연산으로 옮긴다.
//!
//! rope 편집기(`editbufkeys`)와 대응하는 자리지만 연산 이름이 다르다. 이쪽 커서는 문서 전역
//! char 오프셋이 아니라 **바이트 오프셋**이고, 이동은 전부 줄 안에서 재기 때문이다
//! (그래야 CP949 같은 문서에서도 안전하다 — [`crate::textdata::TextData::char_starts`]).

use crate::textbuf::TextBuf;
use egui::{Event, Key};

/// 이 프레임의 입력을 버퍼에 적용한다. `page`는 한 화면에 보이는 줄 수.
///
/// 넣을 수 없는 글자(그 인코딩으로 못 적는 이모지 등)를 만나면 그 사실을 돌려준다 —
/// 화면 쪽에서 "왜 안 들어가는지" 알려 줄 수 있게. 조용히 무시하면 고장으로 보인다.
pub fn handle(ui: &egui::Ui, tb: &mut TextBuf, page: usize) -> bool {
    let events = ui.input(|i| i.events.clone());
    let (mut copy, mut refused) = (None::<String>, false);
    for ev in events {
        match ev {
            Event::Text(t) if !t.is_empty() && !t.chars().any(char::is_control) => {
                refused |= !tb.insert(&t) && !tb.readonly;
            }
            Event::Paste(s) => {
                // 붙여넣기는 한 덩어리로 되돌려야 한다 — 글자마다 취소하게 두지 않는다.
                tb.break_group();
                refused |= !tb.insert(&s.replace("\r\n", "\n")) && !tb.readonly;
                tb.break_group();
            }
            Event::Copy => copy = Some(tb.selected_text()),
            Event::Cut => {
                copy = Some(tb.selected_text());
                if tb.has_selection() {
                    tb.erase(false);
                }
            }
            Event::Key { key, pressed: true, modifiers, .. } => {
                press(tb, key, modifiers, page, &mut copy);
            }
            _ => {}
        }
    }
    if let Some(t) = copy.filter(|t| !t.is_empty()) {
        ui.ctx().copy_text(t);
    }
    refused
}

/// 키 하나. Shift는 선택 확장, Ctrl(command)은 클립보드·되돌리기·문서 끝 이동.
fn press(tb: &mut TextBuf, key: Key, m: egui::Modifiers, page: usize, copy: &mut Option<String>) {
    if m.command {
        match key {
            Key::A => {
                tb.go(0, false);
                tb.go(tb.data.total(), true);
            }
            Key::C => *copy = Some(tb.selected_text()),
            Key::X => {
                *copy = Some(tb.selected_text());
                if tb.has_selection() {
                    tb.erase(false);
                }
            }
            Key::Z if m.shift => tb.redo(),
            Key::Z => tb.undo(),
            Key::Y => tb.redo(),
            Key::Home => tb.go(0, m.shift),
            Key::End => tb.go(tb.data.total(), m.shift),
            _ => {}
        }
        return;
    }
    let sel = m.shift;
    match key {
        Key::ArrowLeft => tb.step(false, sel),
        Key::ArrowRight => tb.step(true, sel),
        Key::ArrowUp => tb.step_line(false, sel),
        Key::ArrowDown => tb.step_line(true, sel),
        Key::Home => tb.go_line_edge(false, sel),
        Key::End => tb.go_line_edge(true, sel),
        Key::PageUp => page_move(tb, page, false, sel),
        Key::PageDown => page_move(tb, page, true, sel),
        Key::Backspace => tb.erase(false),
        Key::Delete => tb.erase(true),
        Key::Enter => tb.insert_newline(),
        Key::Tab => {
            tb.insert("    ");
        }
        _ => {}
    }
}

/// 한 화면만큼 위/아래. 열은 [`TextBuf::step_line`]이 지키는 `goal_col`을 그대로 따른다.
fn page_move(tb: &mut TextBuf, page: usize, down: bool, sel: bool) {
    for _ in 0..page.max(1) {
        tb.step_line(down, sel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textdata::TextData;

    fn buf(s: &str) -> TextBuf {
        TextBuf::new(TextData::from_vec(s.as_bytes().to_vec()))
    }

    /// PageDown이 문서 끝을 넘어가도 터지지 않고 마지막 줄에 선다.
    #[test]
    fn paging_past_the_end_stops_at_the_last_line() {
        let mut b = buf("a\nb\nc");
        page_move(&mut b, 50, true, false);
        assert_eq!(b.caret_line(), 2);
        page_move(&mut b, 50, false, false);
        assert_eq!(b.caret_line(), 0);
    }

    /// 한 화면 크기가 0으로 들어와도 최소 한 줄은 움직인다(무한 대기·정지 방지).
    #[test]
    fn a_zero_sized_page_still_moves_one_line() {
        let mut b = buf("a\nb\nc");
        page_move(&mut b, 0, true, false);
        assert_eq!(b.caret_line(), 1);
    }
}
