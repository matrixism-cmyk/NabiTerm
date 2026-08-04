//! SFTP 패널 상단 툴바(홈·상위·새로고침·주소창·동기화·새폴더·정렬·검색·북마크·재연결 등).
//! render_sftp_tab에서 분리(파일 크기 규율). 액션은 SftpAct에 모은다.

use crate::sftppanel::SftpPanel;
use crate::sftptab::SftpAct;
use nabi_i18n::{tr, Lang};

/// SFTP 툴바를 그리고 클릭 액션을 a에 채운다.
pub(crate) fn render_toolbar(
    ui: &mut egui::Ui,
    sftp: &mut SftpPanel,
    lang: Lang,
    bookmarks: &[String],
    sort_desc: bool,
    a: &mut SftpAct,
) {
    ui.horizontal(|ui| {
        let t = if sftp.host.is_empty() {
            tr(lang, "sftp.title").to_string()
        } else {
            sftp.host.clone()
        };
        ui.label(format!("\u{1f5a7} {t}"));
        if ui.small_button("\u{1f3e0}").on_hover_text("Home").clicked() {
            a.go = Some(".".to_string());
        }
        if ui
            .small_button("\u{2191}")
            .on_hover_text(tr(lang, "browser.up"))
            .clicked()
        {
            a.go = Some(crate::sftppath::parent_dir(&sftp.path));
        }
        if ui
            .small_button("\u{27f3}")
            .on_hover_text(tr(lang, "sftp.refresh"))
            .clicked()
        {
            a.go = Some(sftp.path.clone());
        }
        // 편집 가능한 경로 주소창(FileZilla식 — 직접 입력/붙여넣기로 이동). 비편집 시 현재 경로 표시.
        let r = ui.add(egui::TextEdit::singleline(&mut sftp.addr).desired_width(200.0).hint_text(tr(lang, "sftp.gotopath")));
        if !r.has_focus() {
            sftp.addr = sftp.path.clone();
        } else if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !sftp.addr.trim().is_empty() {
            a.go = Some(sftp.addr.trim().to_string());
        }
        // 디렉터리 동기화(차이 파일만) — WinSCP식. 로컬 브라우저 폴더 기준.
        if ui.small_button("\u{2191}\u{21c5}").on_hover_text(tr(lang, "sftp.syncup")).clicked() { a.sync_up = true; }
        if ui.small_button("\u{2193}\u{21c5}").on_hover_text(tr(lang, "sftp.syncdown")).clicked() { a.sync_down = true; }
        if ui
            .small_button("\u{1f4c1}+")
            .on_hover_text(tr(lang, "sftp.newfolder"))
            .clicked()
        {
            a.new_folder = true;
        }
        if ui
            .small_button("\u{1f4c4}+")
            .on_hover_text(tr(lang, "sftp.newfile"))
            .clicked()
        {
            a.new_file = true;
        }
        if ui.small_button("\u{1f4cb}\u{2193}").on_hover_text(tr(lang, "browser.paste")).clicked() { a.paste = true; } // OS 파일 붙여넣기=업로드(툴바 일관성).
        if ui
            .small_button("\u{2b07}")
            .on_hover_text(tr(lang, "sftp.downloaddir"))
            .clicked()
        {
            a.dl_cur = true;
        }
        // 정렬 방향 표시(▲오름/▼내림). 클릭 시 방향→다음 키 순환(브라우저와 일관).
        if ui
            .small_button(if sort_desc { "\u{25bc}" } else { "\u{25b2}" })
            .on_hover_text(tr(lang, "sftp.sort"))
            .clicked()
        {
            a.cycle_sort = true;
        }
        if ui
            .small_button("\u{21c6}")
            .on_hover_text(tr(lang, "sftp.compare"))
            .clicked()
        {
            a.toggle_compare = true;
        }
        if ui
            .small_button("\u{1f517}")
            .on_hover_text(tr(lang, "sftp.sync"))
            .clicked()
        {
            a.toggle_sync = true;
        }
        if ui
            .small_button("\u{1f50d}")
            .on_hover_text(tr(lang, "sftp.search"))
            .clicked()
        {
            a.search = true;
        }
        if ui
            .small_button("\u{1f524}")
            .on_hover_text(tr(lang, "sftp.batchrename"))
            .clicked()
        {
            a.batch_toggle = true;
        }
        if ui
            .small_button("\u{1f441}")
            .on_hover_text(tr(lang, "sftp.hidden"))
            .clicked()
        {
            sftp.show_hidden = !sftp.show_hidden;
        }
        // 북마크(FileZilla식 즐겨찾기): ⭐로 현재 경로 추가 + 목록에서 이동/삭제.
        ui.menu_button("\u{2b50}", |ui| {
            if ui.button(tr(lang, "sftp.addbookmark")).clicked() { a.bookmark_add = true; ui.close_menu(); }
            for b in bookmarks {
                ui.horizontal(|ui| {
                    if ui.button(b).clicked() { a.bookmark_go = Some(b.clone()); ui.close_menu(); }
                    if ui.small_button("\u{2715}").clicked() { a.bookmark_del = Some(b.clone()); ui.close_menu(); }
                });
            }
        });
        if ui.small_button("\u{1f50c}").on_hover_text(tr(lang, "sftp.reconnect")).clicked() {
            a.reconnect = true;
        }
        if ui.small_button("\u{2715}").clicked() {
            a.close = true;
        }
    });
}
