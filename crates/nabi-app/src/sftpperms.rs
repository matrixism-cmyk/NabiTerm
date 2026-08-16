//! SFTP 권한 변경 메뉴(프리셋·재귀·커스텀 8진수) — sftptable에서 분리.

use crate::sftpentries::EClick;
use crate::sftpentryfmt::{mode_to_rwx, parse_octal_mode};
use nabi_i18n::{tr, Lang};

/// 권한 변경 서브메뉴: 프리셋 + (디렉터리) 하위 포함 재귀 + 커스텀 8진수. 선택 시 Chmod 동작을 돌려준다.
pub(crate) fn perms_menu(ui: &mut egui::Ui, name: &str, is_dir: bool, lang: Lang) -> Option<EClick> {
    let mut click = None;
    for (lbl, m) in [("755", 0o755u32), ("700", 0o700), ("644", 0o644), ("600", 0o600)] {
        if ui.button(lbl).clicked() {
            click = Some(EClick::Chmod(name.to_string(), m));
            ui.close_menu();
        }
    }
    // 디렉터리: 하위 포함 재귀 적용(WinSCP식 "모든 하위에 적용").
    if is_dir {
        ui.menu_button(tr(lang, "sftp.chmodrec"), |ui| {
            for (lbl, m) in [("755", 0o755u32), ("700", 0o700), ("644", 0o644), ("600", 0o600)] {
                if ui.button(lbl).clicked() {
                    click = Some(EClick::ChmodRecursive(name.to_string(), m));
                    ui.close_menu();
                }
            }
        });
    }
    ui.separator();
    // 커스텀 8진수: 프리셋 외 임의 권한(예: 640). 입력은 항목별 임시 메모리에 보관.
    let id = egui::Id::new(("chmod", name));
    let mut txt = ui.data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut txt)
                .hint_text(tr(lang, "sftp.chmodcustom"))
                .desired_width(56.0),
        );
        let parsed = parse_octal_mode(&txt);
        if let Some(m) = parsed {
            ui.weak(mode_to_rwx(m, is_dir, false)); // 유효하면 rwx 미리보기(권한 편집 — 링크 무관).
        }
        if ui.add_enabled(parsed.is_some(), egui::Button::new("\u{2713}")).clicked() {
            if let Some(m) = parsed {
                click = Some(EClick::Chmod(name.to_string(), m));
            }
            ui.close_menu();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, txt));
    click
}
