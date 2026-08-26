//! **바로가기 메뉴**(별표) — 특수 폴더와 다녀온 곳. `browser`가 소프트 한도에 닿아 분리했다.
//!
//! 다녀온 곳은 원격(SFTP)의 별표 메뉴와 **같은 자리·같은 규칙**이다(`recentpaths`).
//! 로컬과 원격에서 찾는 곳이 다르면 사용자는 매번 어느 쪽이었는지 떠올려야 한다.

use crate::browser::{home_dir, BrowserAct};
use nabi_i18n::Lang;

/// 별표 메뉴를 그린다. 고른 것은 `a.nav`로 나간다.
pub(crate) fn places_menu(ui: &mut egui::Ui, lang: Lang, recent: &[String], a: &mut BrowserAct) {
    // 바로가기: 바탕화면/문서/다운로드/네트워크(특수 폴더로 즉시 이동).
    ui.menu_button("\u{2b50}", |ui| {
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
    })
    .response
    .on_hover_text(nabi_i18n::tr(lang, "browser.places"));
}
