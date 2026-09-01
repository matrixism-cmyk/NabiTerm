//! 목록 한 항목의 **색과 상호작용** — 표(자세히)와 격자(아이콘) 양쪽이 함께 쓴다.
//!
//! `browserrows` 에서 떼어냈다. 그쪽은 표를 그리는 일이고 이쪽은 항목 하나를 다루는 일이라
//! 원래 다른 관심사인데, 한 파일에 있으면 표를 고칠 때마다 우클릭 메뉴 백 줄을 지나가야
//! 했다(2026-09-01 줄 수 한도를 넘기면서 갈랐다).

use crate::browserfs::Row;
use crate::browserrows::RowActs;
use crate::sftpview::RemoteName;
use nabi_i18n::{tr, Lang};
use std::collections::HashMap;
use std::path::Path;
/// 유형/비교 색(비교 모드면 비교색 우선, 아니면 폴더=금색·파일=카테고리색).
pub(crate) fn row_color(row: &Row, remote_map: &HashMap<String, (bool, u64)>) -> egui::Color32 {
    if !remote_map.is_empty() {
        let st = crate::sftpentryfmt::cmp_status(&row.name, row.size, row.is_dir, remote_map);
        if let Some(c) = crate::sftpentryfmt::cmp_color(st) {
            return c;
        }
    }
    if row.is_dir {
        crate::filetype::FOLDER_COLOR
    } else {
        crate::filetype::file_color(&row.name)
    }
}

/// 항목 위젯 상호작용: 드래그(업로드 페이로드)·폴더 드롭·더블클릭(열기/진입)·우클릭 메뉴.
/// 테이블/격자 양쪽 셀이 공유한다(click_and_drag 센스 위젯의 응답을 넘긴다).
#[allow(clippy::too_many_arguments)]
pub(crate) fn row_interact(
    ui: &egui::Ui,
    resp: &egui::Response,
    row: &Row,
    path: &Path,
    can_upload: bool,
    lang: Lang,
    is_selected: bool,
    acts: &mut RowActs,
) {
    // 드래그 시작 → OS 드래그-아웃(탐색기로 복사). 앱 내 SFTP 패널 드롭도 OS 경로로 처리.
    if resp.drag_started() {
        acts.os_drag = Some(row.name.clone());
    }
    if row.is_dir {
        // 드롭 가능한 폴더에 드래그가 올라오면 테두리로 강조(드롭 대상 안내).
        if resp.dnd_hover_payload::<RemoteName>().is_some() {
            let c = ui.visuals().selection.stroke.color;
            ui.painter().rect_stroke(resp.rect, 3.0, egui::Stroke::new(2.0, c), egui::StrokeKind::Inside);
        }
        if let Some(rn) = resp.dnd_release_payload::<RemoteName>() {
            acts.dl_into = Some((row.name.clone(), (*rn).clone()));
        }
    }
    if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        acts.select = Some((row.name.clone(), m.command, m.shift)); // 선택(Ctrl=토글, Shift=범위).
    }
    if resp.double_clicked() {
        if row.is_dir {
            acts.nav = Some(path.join(&row.name));
        } else {
            crate::paneurl::os_open(&path.join(&row.name).to_string_lossy());
        }
    }
    // 우클릭: 아직 선택 안 된 항목이면 그 항목만 선택. 이미 선택(다중 포함)돼 있으면
    // 기존 선택을 유지한다(여러 개 선택 후 우클릭으로 일괄 동작하도록).
    if resp.secondary_clicked() && !is_selected {
        acts.select = Some((row.name.clone(), false, false));
    }
    resp.context_menu(|ui| {
        let full = path.join(&row.name);
        // 파일이면 편집(내장 에디터 — 설정에 따라 외부도). 폴더는 제외.
        if !row.is_dir && ui.button(tr(lang, "sftp.edit")).clicked() {
            acts.edit = Some(row.name.clone());
            ui.close();
        }
        if !row.is_dir && ui.button(tr(lang, "nabipad.openhex")).clicked() { acts.edit_hex = Some(row.name.clone()); ui.close(); }
        if !row.is_dir && ui.button(tr(lang, "browser.preview")).clicked() { acts.preview = Some(row.name.clone()); ui.close(); } // E9
        // CF_HDROP 복사 — 탐색기에서 Ctrl+V로 실제 파일 붙여넣기.
        if ui.button(tr(lang, "browser.copy")).clicked() {
            acts.copy = Some(row.name.clone());
            ui.close();
        }
        if ui.button(tr(lang, "browser.copypath")).clicked() {
            ui.ctx().copy_text(full.to_string_lossy().into_owned());
            ui.close();
        }
        if ui.button(tr(lang, "browser.reveal")).clicked() {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", full.display()))
                .spawn();
            ui.close();
        }
        if ui.button(tr(lang, "browser.props")).clicked() {
            acts.props = Some(row.name.clone());
            ui.close();
        }
        if row.is_dir && ui.button(tr(lang, "browser.calcsize")).clicked() {
            acts.calc_size = Some(row.name.clone());
            ui.close();
        }
        if ui.button(tr(lang, "browser.duplicate")).clicked() {
            acts.duplicate = Some(row.name.clone());
            ui.close();
        }
        // 압축은 한 묶음으로 — 항목 둘을 최상위에 늘어놓으면 이 메뉴가 또 길어진다.
        ui.menu_button(tr(lang, "browser.zipmenu"), |ui| {
            if ui.button(tr(lang, "browser.zipmake")).clicked() {
                acts.zip_make = Some(row.name.clone());
                ui.close();
            }
            // 푸는 것은 zip일 때만 보인다 — 아닌 파일에 보이면 눌러 놓고 왜 안 되는지 묻는다.
            if row.name.to_ascii_lowercase().ends_with(".zip") && ui.button(tr(lang, "browser.zipextract")).clicked() {
                acts.zip_extract = Some(row.name.clone());
                ui.close();
            }
        });
        if ui.button(tr(lang, "sftp.rename")).clicked() {
            acts.rename = Some(row.name.clone());
            ui.close();
        }
        if ui.button(tr(lang, "browser.delete")).clicked() {
            acts.delete = Some(row.name.clone());
            ui.close();
        }
        if can_upload && !row.is_dir && ui.button(tr(lang, "browser.upload")).clicked() {
            acts.upload = Some(row.name.clone());
            ui.close();
        }
    });
}
