//! 전송·SFTP 설정 화면(배치 AM에서 settingsui2.rs 줄 한도로 갈라 냈다).
//!
//! 세션 기록을 터미널 페이지로 옮기면서도 settingsui2.rs 가 한도를 넘어 있었다.
//! 한 페이지가 통째로 들어 있으니 그 단위로 가르는 것이 가장 자연스럽다.

use nabi_config::AppConfig;
use nabi_i18n::{tr, Lang};

/// 전송·SFTP 페이지 — 속도/병렬/무결성/파일명 인코딩 + 다운로드 폴더를 한자리에.
pub(crate) fn transfer_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    sftp_rows(ui, cfg, lang);
    crate::settingsui::grid_seg(ui, "sec_transfer_dl", |ui| {
    // SFTP 다운로드 기본 폴더(비우면 로컬 창/홈) + 매번 물어보기 여부.
    crate::settingsui::label_cell(ui, tr(lang, "settings.downloaddir"));
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut cfg.terminal.download_dir)
            .desired_width(220.0)
            .hint_text(tr(lang, "settings.downloaddirhint"));
        ui.add(edit);
        if ui.button("\u{1f4c1}").clicked() {
            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                cfg.terminal.download_dir = d.to_string_lossy().into_owned();
            }
        }
    });
    ui.end_row();
    // 업로드 권한 정규화 — 빈 값=끄기(기본). auto면 스크립트에 실행 비트.
    crate::settingsui::label_cell(ui, tr(lang, "settings.uploadmode"));
    ui.add(egui::TextEdit::singleline(&mut cfg.terminal.sftp_upload_mode).desired_width(120.0).hint_text("off / auto / 644"))
        .on_hover_text(tr(lang, "settings.uploadmodehint"));
    ui.end_row();
    crate::settingsui::label_cell(ui, tr(lang, "settings.downloadask"));
    ui.checkbox(&mut cfg.terminal.download_ask, tr(lang, "settings.downloadaskhint"));
    ui.end_row();
    });
}

/// SFTP 전송·파일명 인코딩 그룹(전송 페이지에서 호출 — settingsui.rs 라인 한도로 분리).
fn sftp_rows(ui: &mut egui::Ui, cfg: &mut AppConfig, lang: Lang) {
    crate::settingsui::grid_seg(ui, "sec_sftp_a", |ui| {
    // 외부 편집기 — 원격 파일을 밖에서 열 때. 비우면 기존처럼 OS 기본 앱.
    crate::settingsui::label_cell(ui, tr(lang, "settings.exteditor")); ui.add(egui::TextEdit::singleline(&mut cfg.terminal.external_editor).hint_text("code").desired_width(160.0))
        .on_hover_text(tr(lang, "settings.exteditorhint"));
    ui.end_row();
    crate::settingsui::label_cell(ui, tr(lang, "settings.speedlimit"));
    ui.add(egui::DragValue::new(&mut cfg.terminal.speed_limit_kbps).suffix(" KB/s")); ui.end_row();
    crate::settingsui::label_cell(ui, tr(lang, "settings.maxparallel"));
    ui.add(egui::Slider::new(&mut cfg.terminal.max_parallel_transfers, 1..=4));
    ui.end_row();
    crate::settingsui::label_cell(ui, tr(lang, "settings.verifyhash"));
    ui.checkbox(&mut cfg.terminal.sftp_verify_hash, "").on_hover_text(tr(lang, "settings.verifyhashhint"));
    ui.end_row();
    // 선택 열은 **설정이 아니라 목록에서** 고른다(머리글 오른쪽 클릭 — 탐색기와 같다).
    // 여기서는 그런 길이 있다는 것만 알린다. 설정 화면까지 와서 찾은 사람이 헛걸음하지
    // 않도록, 그리고 목록에서 오른쪽 클릭을 해 볼 생각을 못 한 사람에게 알리려고 둔다.
    crate::settingsui::label_cell(ui, tr(lang, "settings.extracol"));
    ui.label("");
    ui.end_row();
    });
    // 설명은 표 밖에서 두 칸을 가로지른다 — 칸 안에 두면 그 칸이 설명 길이만큼 넓어져
    // 설정 창 전체가 늘어난다(사용자 보고 2026-09-05).
    crate::settingsui::help_line(ui, tr(lang, "settings.extracolhint"));
    crate::settingsui::grid_seg(ui, "sec_sftp_b", |ui| {
    // SFTP 파일명 인코딩 — v3 서버가 로컬 인코딩 raw 바이트로 보낼 때(한국 서버 CP949 등).
    crate::settingsui::label_cell(ui, tr(lang, "settings.sftpcharset"));
    egui::ComboBox::from_id_salt("sftp_name_charset")
        .selected_text(if cfg.terminal.sftp_name_charset == "auto" { tr(lang, "settings.sftpcharset.auto").to_string() } else { cfg.terminal.sftp_name_charset.clone() })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut cfg.terminal.sftp_name_charset, "auto".into(), tr(lang, "settings.sftpcharset.auto"));
            for e in ["utf8", "euc-kr", "shift_jis", "gbk"] {
                ui.selectable_value(&mut cfg.terminal.sftp_name_charset, e.into(), e);
            }
        })
        .response
        .on_hover_text(tr(lang, "settings.sftpcharsethint"));
    ui.end_row();
    });
}
