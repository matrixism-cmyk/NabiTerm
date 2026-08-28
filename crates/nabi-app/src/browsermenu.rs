//! 로컬 브라우저의 빈 공간 우클릭 메뉴. 목록 행 메뉴는 browserrows.
//!
//! ⚠️ 이 메뉴는 **빈 영역에만** 붙여야 한다. 채움 컨테이너 응답에 `.context_menu()`를 달면
//! 자식 행의 왼쪽 클릭이 먹통이 된다(egui 0.29 확인).

use crate::browser::BrowserAct;
use nabi_i18n::Lang;
/// 파일 목록 빈 공간 우클릭 메뉴 — 새 폴더/파일·붙여넣기·여기서 터미널·숨김 토글·경로 복사(툴바와 동일 동작).
pub(crate) fn empty_space_menu(ui: &mut egui::Ui, a: &mut BrowserAct, lang: Lang, path: &std::path::Path) {
    let tr = |k| nabi_i18n::tr(lang, k);
    if ui.button(format!("\u{1f4c1}+ {}", tr("sftp.newfolder"))).clicked() { a.new_folder = true; ui.close(); }
    if ui.button(format!("\u{1f4c4}+ {}", tr("sftp.newfile"))).clicked() { a.new_file = true; ui.close(); }
    if ui.button(format!("\u{1f4cb}\u{2193} {}", tr("browser.paste"))).clicked() { a.paste = true; ui.close(); }
    ui.separator();
    if ui.button(format!("\u{1f4bb} {}", tr("browser.termhere"))).clicked() { a.term_here = true; ui.close(); }
    if ui.button(format!("\u{1f441} {}", tr("sftp.hidden"))).clicked() { a.toggle_hidden = true; ui.close(); }
    if ui.button(format!("\u{1f4cb} {}", tr("browser.copycurpath"))).clicked() { ui.ctx().copy_text(path.to_string_lossy().into_owned()); ui.close(); }
    ui.separator();
    // 선택·분석 도구는 "도구" 서브메뉴로 묶어 최상위를 간결하게(메뉴 정리).
    ui.menu_button(tr("editor.toolsmenu"), |ui| {
        if ui.button(format!("\u{2611} {}", tr("menu.selectall"))).clicked() { a.select_all = true; ui.close(); }
        if ui.button(format!("\u{21c4} {}", tr("menu.invertsel"))).clicked() { a.invert_sel = true; ui.close(); }
        if ui.button(format!("\u{1f4cb} {}", tr("menu.copypaths"))).clicked() { a.copy_paths = true; ui.close(); }
        // 선택한 파일들을 한꺼번에 — 새 메뉴를 만들지 않고 이미 있는 도구 묶음에 넣는다.
        if ui.button(format!("\u{270e} {}", tr("browser.batchrename"))).clicked() { a.batch_rename = true; ui.close(); }
        ui.separator();
        if ui.button(format!("\u{1f333} {}", tr("dir.tree"))).clicked() { a.dir_tree = true; ui.close(); }
        if ui.button(format!("\u{1f4ca} {}", tr("dir.stats"))).clicked() { a.dir_stats = true; ui.close(); }
    });
}
