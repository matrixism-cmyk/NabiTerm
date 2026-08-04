//! egui 키/텍스트 이벤트 → PTY 바이트 변환.
//!
//! 일반 문자는 Event::Text가 처리하고, 특수키/Ctrl 조합만 여기서 시퀀스로 변환해
//! 이중 입력을 피한다.

use egui::Event;

/// 포커스된 터미널에서 OS IME를 활성화한다(한/영 전환·CJK 조합) — egui는 ime 출력이
/// 있어야만 창에 IME를 허용하므로, 매 프레임 커서 셀 위치로 광고한다(조합창 위치 겸용).
pub fn advertise_ime(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    model: &nabi_vt::TermModel,
    cw: f32,
    ch: f32,
) {
    let cur = model.cursor();
    let min = rect.min + egui::vec2(f32::from(cur.col) * cw, f32::from(cur.row) * ch);
    let crect = egui::Rect::from_min_size(min, egui::vec2(cw, ch));
    ui.output_mut(|o| {
        o.ime = Some(egui::output::IMEOutput {
            rect: crect,
            cursor_rect: crect,
        });
    });
}

/// IME 조합 상태(preedit)를 이벤트로 갱신한다. 조합 중 텍스트(초성·중성·종성
/// 진행)는 `Preedit`로 오고, 확정되면 `Commit`(events_to_bytes가 PTY로 전송)이라
/// 여기서 비운다. 반환값은 조합 문자열이 바뀌었는지(리페인트 트리거용).
pub fn update_preedit(events: &[Event], preedit: &mut String) -> bool {
    let before = preedit.clone();
    for ev in events {
        match ev {
            Event::Ime(egui::ImeEvent::Preedit(s)) => s.clone_into(preedit),
            Event::Ime(egui::ImeEvent::Commit(_)) | Event::Ime(egui::ImeEvent::Disabled) => {
                preedit.clear();
            }
            _ => {}
        }
    }
    *preedit != before
}

/// 한 프레임의 입력 이벤트를 PTY로 보낼 바이트로 변환한다.
///
/// `app_cursor`(DECCKM)가 켜지면 방향키/Home/End가 `ESC O _` 시퀀스가 된다(vim/less).
/// `bracketed_paste`(DECSET 2004)가 켜지면 붙여넣기를 `ESC[200~…ESC[201~`로 감싼다.
/// `composing`(IME 조합 중)이면 키 입력을 PTY로 보내지 않는다 — Backspace 등은 IME가
/// 처리(초성·중성·종성 분해)해야 하므로(아니면 조합 전체가 한 번에 사라짐).
pub fn events_to_bytes(
    events: &[Event],
    app_cursor: bool,
    bracketed_paste: bool,
    mut composing: bool,
) -> Vec<u8> {
    let mut out = Vec::new();
    // 이벤트를 순서대로 처리하며 조합 상태를 갱신한다 — 확정(Commit) 이후 '같은 프레임'의
    // Enter 등 키는 PTY로 보내야 한다(아니면 한글로 끝나는 명령에서 엔터를 두 번 눌러야 함).
    for ev in events {
        match ev {
            Event::Text(t) => out.extend_from_slice(t.as_bytes()),
            // CJK IME 확정 텍스트는 Text가 아니라 Ime(Commit)로 온다(한/일 입력). 확정 후 조합 종료.
            Event::Ime(egui::ImeEvent::Commit(t)) => {
                out.extend_from_slice(t.as_bytes());
                composing = false;
            }
            Event::Ime(egui::ImeEvent::Preedit(s)) if !s.is_empty() => composing = true,
            Event::Ime(egui::ImeEvent::Disabled) => composing = false,
            Event::Paste(s) => {
                // paste-injection 방지: 끼어든 bracketed 마커 제거 + 위험 제어문자 위생.
                let clean = crate::paste::sanitize_paste(
                    &s.replace("\x1b[200~", "").replace("\x1b[201~", ""),
                );
                if bracketed_paste {
                    out.extend_from_slice(b"\x1b[200~");
                    out.extend_from_slice(clean.as_bytes());
                    out.extend_from_slice(b"\x1b[201~");
                } else {
                    out.extend_from_slice(clean.as_bytes());
                }
            }
            // 조합 중에는 키를 IME에 양보(PTY로 보내지 않음).
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if !composing => {
                if let Some(seq) = crate::keymap::key_to_bytes(*key, *modifiers, app_cursor) {
                    out.extend_from_slice(&seq);
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers};

    #[test]
    fn arrow_keys_respect_application_cursor_mode() {
        let up = Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(events_to_bytes(std::slice::from_ref(&up), false, false, false), b"\x1b[A");
        assert_eq!(events_to_bytes(std::slice::from_ref(&up), true, false, false), b"\x1bOA");
    }

    #[test]
    fn alt_backspace_is_meta_del() {
        let ev = Event::Key {
            key: Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                alt: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(events_to_bytes(std::slice::from_ref(&ev), false, false, false), vec![0x1bu8, 0x7f]);
    }

    #[test]
    fn ctrl_space_emits_nul() {
        let ev = Event::Key {
            key: Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(events_to_bytes(std::slice::from_ref(&ev), false, false, false), vec![0u8]);
    }

    #[test]
    fn modified_arrows_and_backtab() {
        let mk = |k: Key, m: Modifiers| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: m,
        };
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let shift = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        // Ctrl+Right = CSI 1;5C(단어 점프).
        assert_eq!(
            events_to_bytes(&[mk(Key::ArrowRight, ctrl)], false, false, false),
            b"\x1b[1;5C"
        );
        // Shift+Tab = CSI Z(backtab).
        assert_eq!(events_to_bytes(&[mk(Key::Tab, shift)], false, false, false), b"\x1b[Z");
    }

    #[test]
    fn function_keys_emit_sequences() {
        let mk = |k: Key| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(events_to_bytes(&[mk(Key::F1)], false, false, false), b"\x1bOP");
        assert_eq!(events_to_bytes(&[mk(Key::F5)], false, false, false), b"\x1b[15~");
        assert_eq!(events_to_bytes(&[mk(Key::F12)], false, false, false), b"\x1b[24~");
    }

    #[test]
    fn keys_suppressed_while_composing() {
        // 조합 중 Backspace는 PTY로 가지 않는다(IME가 초성·중성·종성 분해 처리).
        let bs = Event::Key {
            key: Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(events_to_bytes(std::slice::from_ref(&bs), false, false, true), Vec::<u8>::new());
        assert_eq!(events_to_bytes(std::slice::from_ref(&bs), false, false, false), vec![0x7f]);
    }

    #[test]
    fn commit_then_enter_sends_newline_same_frame() {
        // 직전 프레임 조합 중(composing=true)이어도, Commit 직후 같은 프레임 Enter는 \r로 보낸다.
        let commit = Event::Ime(egui::ImeEvent::Commit("가".into()));
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(events_to_bytes(&[commit, enter], false, false, true), "가\r".as_bytes());
    }

    #[test]
    fn preedit_suppresses_following_key_same_frame() {
        // 조합(Preedit) 진행 중 같은 프레임의 Enter는 PTY로 보내지 않는다(IME가 처리).
        let pre = Event::Ime(egui::ImeEvent::Preedit("가".into()));
        let enter = Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(events_to_bytes(&[pre, enter], false, false, false), Vec::<u8>::new());
    }

    #[test]
    fn paste_is_bracketed_when_enabled() {
        let ev = Event::Paste("hi".to_string());
        assert_eq!(events_to_bytes(std::slice::from_ref(&ev), false, false, false), b"hi");
        assert_eq!(
            events_to_bytes(std::slice::from_ref(&ev), false, true, false),
            b"\x1b[200~hi\x1b[201~"
        );
    }

    #[test]
    fn bracketed_paste_strips_injected_end_marker() {
        // 내용에 끼어든 ESC[200~/201~는 모두 제거되어 조기 종료/주입을 막는다.
        let ev = Event::Paste("a\x1b[200~b\x1b[201~rm -rf".to_string());
        assert_eq!(
            events_to_bytes(std::slice::from_ref(&ev), false, true, false),
            b"\x1b[200~abrm -rf\x1b[201~"
        );
    }
}
