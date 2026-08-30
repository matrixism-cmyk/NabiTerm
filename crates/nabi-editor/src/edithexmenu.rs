//! HEX 편집기 우클릭 컨텍스트 메뉴 — 잘 만든 HEX 에디터식 동작.
//! 블록 선택 시: 복사(HEX/텍스트)·잘라내기·삭제. 항상: 붙여넣기(HEX/텍스트 삽입)·바이트 삽입·전체 선택.

use crate::edithex::HexBuf;
use crate::edithexedit::{parse_hex, to_hex_string};
use nabi_i18n::{tr, Lang};

/// 우클릭 메뉴 본문. 선택 여부·읽기 전용에 따라 항목을 노출한다.
pub fn context_menu(
    ui: &mut egui::Ui,
    h: &mut HexBuf,
    readonly: bool,
    lang: Lang,
    failed: &mut Option<String>,
) {
    if !readonly {
        ui.add_enabled_ui(!h.undo.is_empty(), |ui| {
            if ui.button(tr(lang, "editor.undo")).clicked() { h.undo(); ui.close(); }
        });
        ui.add_enabled_ui(!h.redo.is_empty(), |ui| {
            if ui.button(tr(lang, "editor.redo")).clicked() { h.redo(); ui.close(); }
        });
        ui.separator();
    }
    if h.selection().is_some() {
        // 복사 형식(HEX/텍스트/C 배열/Base64)을 한 서브메뉴로 묶는다.
        ui.menu_button(tr(lang, "nabipad.copygroup"), |ui| {
            if ui.button(tr(lang, "nabipad.copyhex")).clicked() {
                ui.ctx().copy_text(to_hex_string(&h.selected_bytes()));
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.copytext")).clicked() {
                ui.ctx().copy_text(ascii_of(&h.selected_bytes()));
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.copycarray")).clicked() {
                ui.ctx().copy_text(crate::edithexedit::to_c_array(&h.selected_bytes()));
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.copyb64")).clicked() {
                ui.ctx().copy_text(crate::editorconvert::base64_encode_bytes(&h.selected_bytes()));
                ui.close();
            }
        });
        if !readonly && ui.button(tr(lang, "nabipad.cut")).clicked() {
            ui.ctx().copy_text(to_hex_string(&h.selected_bytes()));
            h.delete_forward(); // 선택 삭제.
            ui.close();
        }
        if !readonly && ui.button(tr(lang, "nabipad.delete")).clicked() {
            h.delete_forward();
            ui.close();
        }
        if !readonly {
            // 채우기 값(0x00/0xFF)을 한 서브메뉴로 묶는다.
            ui.menu_button(tr(lang, "nabipad.fillgroup"), |ui| {
                if ui.button(tr(lang, "nabipad.fill00")).clicked() {
                    h.fill_selection(0x00);
                    ui.close();
                }
                if ui.button(tr(lang, "nabipad.fillff")).clicked() {
                    h.fill_selection(0xFF);
                    ui.close();
                }
            });
        }
        if ui.button(tr(lang, "nabipad.exportsel")).clicked() {
            let data = h.selected_bytes();
            if let Some(path) = rfd::FileDialog::new().set_file_name("selection.bin").save_file() {
                // 선택 바이트를 파일로 추출. 실패하면 앱이 알린다.
                if let Err(e) = std::fs::write(path, data) {
                    *failed = Some(e.to_string());
                }
            }
            ui.close();
        }
        ui.separator();
    }
    if !readonly {
        // 붙여넣기/삽입 형식을 한 서브메뉴로 묶어 컨텍스트 메뉴를 정돈한다.
        ui.menu_button(tr(lang, "nabipad.pastegroup"), |ui| {
            if ui.button(tr(lang, "nabipad.pastehex")).clicked() {
                if let Some(t) = crate::uiutil::clipboard_text() {
                    h.insert_bytes(&parse_hex(&t)); // 클립보드를 HEX로 해석해 삽입.
                }
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.pastetext")).clicked() {
                if let Some(t) = crate::uiutil::clipboard_text() {
                    h.insert_bytes(t.as_bytes()); // 클립보드 텍스트를 바이트로 삽입.
                }
                ui.close();
            }
            if ui.button(tr(lang, "nabipad.insertbyte")).clicked() {
                h.insert_bytes(&[0]); // 0x00 한 바이트 삽입(이후 편집).
                ui.close();
            }
        });
        ui.separator();
    }
    if !readonly {
        // 비트 연산(선택 영역, 없으면 전체) — HxD/010 식.
        ui.menu_button(tr(lang, "nabipad.bitops"), |ui| {
            if ui.button(tr(lang, "nabipad.invert")).clicked() { h.invert(); ui.close(); }
            if ui.button(tr(lang, "nabipad.swapnib")).clicked() { h.swap_nibbles(); ui.close(); }
            if ui.button(tr(lang, "nabipad.revbytes")).clicked() { h.reverse_bytes(); ui.close(); }
            if ui.button(tr(lang, "nabipad.shl")).clicked() { h.shift_left(); ui.close(); }
            if ui.button(tr(lang, "nabipad.shr")).clicked() { h.shift_right(); ui.close(); }
            if ui.button(tr(lang, "nabipad.rol")).clicked() { h.rotate_left(); ui.close(); }
            if ui.button(tr(lang, "nabipad.ror")).clicked() { h.rotate_right(); ui.close(); }
            if ui.button(tr(lang, "nabipad.revbits")).clicked() { h.reverse_bits(); ui.close(); }
            if ui.button(tr(lang, "nabipad.swap16")).clicked() { h.swap_bytes16(); ui.close(); }
            if ui.button(tr(lang, "nabipad.swap32")).clicked() { h.swap_bytes32(); ui.close(); }
            if ui.button(tr(lang, "nabipad.incbyte")).clicked() { h.increment(); ui.close(); }
            if ui.button(tr(lang, "nabipad.decbyte")).clicked() { h.decrement(); ui.close(); }
        });
    }
    if ui.button(tr(lang, "tab.selectall")).clicked() {
        h.select_all();
        ui.close();
    }
}

/// 바이트열을 ASCII 보기 문자열로(인쇄 불가는 '.').
fn ascii_of(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect()
}
