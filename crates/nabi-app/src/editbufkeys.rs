//! 편집 버퍼(E6) 키보드/클립보드 입력 처리. egui 이벤트를 편집 연산으로 옮긴다.
//! Text=삽입, 방향/Home/End/PageUp·Down=이동, Backspace/Delete/Enter/Tab=편집,
//! Ctrl+A/C/X/V/Z/Y + egui Copy/Cut/Paste 이벤트 처리. 읽기 전용이면 변경 연산은 건너뛴다.

use crate::editbuf::EditBuf;
use egui::{Event, Key};

/// 한 프레임의 입력 이벤트를 편집 버퍼에 적용한다. page=한 화면 줄 수.
pub(crate) fn apply_keys(ui: &egui::Ui, eb: &mut EditBuf, page: i64, readonly: bool) {
    let events = ui.input(|i| i.events.clone());
    let mut copy: Option<String> = None;
    for ev in events {
        match ev {
            Event::Text(t) if !t.is_empty() && !t.chars().any(|c| c.is_control()) => {
                if !readonly {
                    eb.insert(&t);
                }
            }
            Event::Paste(s) => {
                if !readonly {
                    eb.insert(&s);
                }
            }
            Event::Copy => copy = Some(eb.selected_text()),
            Event::Cut => {
                copy = Some(eb.selected_text());
                if !readonly && eb.selection().is_some() {
                    eb.backspace(); // 선택 삭제.
                }
            }
            Event::Key { key, pressed: true, modifiers, .. } => {
                key_press(eb, key, modifiers, page, readonly, &mut copy);
            }
            _ => {}
        }
    }
    if let Some(t) = copy {
        if !t.is_empty() {
            ui.ctx().copy_text(t);
        }
    }
}

/// 단일 키 처리. 이동은 shift로 선택 확장. 편집/붙여넣기는 readonly면 무시.
fn key_press(eb: &mut EditBuf, key: Key, m: egui::Modifiers, page: i64, readonly: bool, copy: &mut Option<String>) {
    let sel = m.shift;
    // 편집/클립보드 단축키(command=Ctrl/⌘).
    if m.command {
        match key {
            Key::A => eb.select_all(),
            Key::C => *copy = Some(eb.selected_text()),
            Key::X => {
                *copy = Some(eb.selected_text());
                if !readonly && eb.selection().is_some() {
                    eb.backspace();
                }
            }
            Key::Z if !readonly && !m.shift => eb.undo(),
            Key::Z if !readonly && m.shift => eb.redo(),
            Key::Y if !readonly => eb.redo(),
            Key::ArrowLeft => eb.move_word(false, m.shift), // Ctrl+← 단어 단위(Shift=선택).
            Key::ArrowRight => eb.move_word(true, m.shift),
            Key::Backspace if !readonly => eb.delete_word(false), // Ctrl+Backspace 단어 삭제.
            Key::Delete if !readonly => eb.delete_word(true),
            _ => {}
        }
        return;
    }
    match key {
        Key::ArrowLeft => eb.move_h(-1, sel),
        Key::ArrowRight => eb.move_h(1, sel),
        Key::ArrowUp => eb.move_v(-1, sel),
        Key::ArrowDown => eb.move_v(1, sel),
        Key::Home => eb.home(sel),
        Key::End => eb.end(sel),
        Key::PageUp => eb.move_v(-page, sel),
        Key::PageDown => eb.move_v(page, sel),
        Key::Backspace if !readonly => eb.backspace(),
        Key::Delete if !readonly => eb.delete(),
        Key::Enter if !readonly => eb.insert_newline(), // 자동 들여쓰기.
        Key::Tab if !readonly => eb.insert_indent(), // 설정(탭/공백)에 따라.
        _ => {}
    }
}
