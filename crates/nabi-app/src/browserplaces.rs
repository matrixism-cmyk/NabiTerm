//! **바로가기 메뉴**(별표) — 특수 폴더·다녀온 곳·꽂아 둔 곳.
//!
//! 다녀온 곳은 원격(SFTP)의 별표 메뉴와 **같은 자리·같은 규칙**이다(`recentpaths`).
//! 로컬과 원격에서 찾는 곳이 다르면 사용자는 매번 어느 쪽이었는지 떠올려야 한다.
//!
//! ## 즐겨찾기는 오래 한쪽에만 있었다
//!
//! 원격 별표에는 "현재 경로를 즐겨찾기에 추가"가 처음부터 있었는데 로컬에는 없었다
//! (2026-09-10에 두 메뉴를 나란히 세어 보고 알았다). 로컬에는 바탕화면·문서·다운로드처럼
//! **정해진 곳**과 다녀온 곳만 있어서, 자주 가는 작업 폴더를 스스로 꽂아 둘 자리가 없었다.
//! 다녀온 곳은 저절로 밀려 나가므로 그 자리를 대신하지 못한다.
//!
//! 차례도 원격(`sftptoolbar::bookmark_menu`)과 같게 맞췄다 — 추가가 맨 위, 다녀온 곳,
//! 그다음 꽂아 둔 곳.

use crate::browser::{home_dir, BrowserAct};
use nabi_i18n::Lang;

/// 별표 메뉴를 그린다. 고른 것은 `a.nav`로 나간다.
pub(crate) fn places_menu(
    ui: &mut egui::Ui,
    lang: Lang,
    recent: &[String],
    bookmarks: &[String],
    a: &mut BrowserAct,
) {
    // 바로가기: 바탕화면/문서/다운로드/네트워크(특수 폴더로 즉시 이동).
    ui.menu_button("\u{2b50}", |ui| {
        if ui.button(nabi_i18n::tr(lang, "sftp.addbookmark")).clicked() {
            a.bookmark_add = true;
            ui.close();
        }
        ui.separator();
        let home = home_dir();
        for (key, sub) in [("browser.desktop", "Desktop"), ("browser.documents", "Documents"), ("browser.downloads", "Downloads")] {
            if ui.button(nabi_i18n::tr(lang, key)).clicked() {
                a.nav = Some(home.join(sub));
                ui.close();
            }
        }
        if ui.button(nabi_i18n::tr(lang, "browser.network")).clicked() {
            // 네트워크는 인앱 SMB 열거가 없어 OS 네트워크 폴더로 연다.
            let _ = std::process::Command::new("explorer").arg("shell:NetworkPlacesFolder").spawn();
            ui.close();
        }
        // 다녀온 곳 — 원격(SFTP)의 별표 메뉴와 같은 자리, 같은 규칙.
        if !recent.is_empty() {
            ui.separator();
            ui.weak(nabi_i18n::tr(lang, "sftp.recent"));
            for r in recent.iter() {
                if ui.button(r).clicked() {
                    a.nav = Some(std::path::PathBuf::from(crate::recentpaths::path_of(r)));
                    ui.close();
                }
            }
        }
        // 꽂아 둔 곳 — 줄 오른쪽의 ✕ 로 뺀다(원격과 같은 모양).
        if !bookmarks.is_empty() {
            ui.separator();
            ui.weak(nabi_i18n::tr(lang, "sftp.bookmarks"));
        }
        for b in bookmarks {
            ui.horizontal(|ui| {
                if ui.button(b).clicked() {
                    a.nav = Some(std::path::PathBuf::from(b));
                    ui.close();
                }
                if ui.small_button("\u{2715}").clicked() {
                    a.bookmark_del = Some(b.clone());
                    ui.close();
                }
            });
        }
    })
    .response
    .on_hover_text(nabi_i18n::tr(lang, "browser.places"));
}
