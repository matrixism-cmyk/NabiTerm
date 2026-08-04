//! 로컬 브라우저의 입력 행(새 폴더/파일, 이름변경) 렌더 + 키보드 탐색 — browser.rs에서 분리.

use crate::browserfs::Row;
use nabi_i18n::{tr, Lang};
use std::path::{Path, PathBuf};

impl crate::app::NabiApp {
    /// 마우스 이전/다음(엄지 버튼, Extra1/2)으로 브라우저 히스토리 이동. over=브라우저 위.
    pub(crate) fn browser_history_nav(&mut self, ctx: &egui::Context, over: bool) {
        if !over {
            return;
        }
        let (back, fwd) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Extra1),
                i.pointer.button_pressed(egui::PointerButton::Extra2),
            )
        });
        if back {
            if let Some(p) = self.browser.back.pop() {
                self.browser.fwd.push(self.browser.path.clone());
                self.browser.selected = None;
                self.browser.path = p.clone();
                self.sync_after_local_nav(&p);
            }
        }
        if fwd {
            if let Some(p) = self.browser.fwd.pop() {
                self.browser.back.push(self.browser.path.clone());
                self.browser.selected = None;
                self.browser.path = p.clone();
                self.sync_after_local_nav(&p);
            }
        }
    }
}

/// 보기 모드 콤보(자세히/목록/큰·작은 아이콘/타일).
pub(crate) fn view_combo(ui: &mut egui::Ui, lang: Lang, view: &mut crate::sftpview::ViewMode) {
    use crate::sftpview::ViewMode;
    egui::ComboBox::from_id_salt("browser_view")
        .selected_text(tr(lang, view.key()))
        .show_ui(ui, |ui| {
            for m in [
                ViewMode::Details,
                ViewMode::List,
                ViewMode::LargeIcons,
                ViewMode::SmallIcons,
                ViewMode::Tile,
            ] {
                ui.selectable_value(view, m, tr(lang, m.key()));
            }
        });
}

/// 키보드 탐색(↑↓ 선택 이동·Enter 열기/진입·Backspace 상위). 폴더 진입/상위 이동 경로를 돌려준다.
/// 브라우저에 포인터가 있고 입력 위젯/팝업 포커스가 없을 때만 동작(터미널 입력과 분리).
pub(crate) fn keyboard_nav(
    ctx: &egui::Context,
    over: bool,
    entries: &[Row],
    filter: &str,
    path: &Path,
    selected: &mut Option<String>,
    scroll: &mut bool,
) -> Option<PathBuf> {
    if !over || ctx.memory(|m| m.focused().is_some() || m.any_popup_open()) {
        return None;
    }
    let filt = filter.to_lowercase();
    let vis: Vec<&Row> = entries
        .iter()
        .filter(|r| crate::browserfs::name_matches(&filt, &r.name))
        .collect();
    let cur = selected
        .as_ref()
        .and_then(|s| vis.iter().position(|r| &r.name == s));
    let n0 = egui::Modifiers::NONE;
    let (down, up, enter, back, home, end, pgup, pgdn) = ctx.input_mut(|i| {
        (
            i.consume_key(n0, egui::Key::ArrowDown),
            i.consume_key(n0, egui::Key::ArrowUp),
            i.consume_key(n0, egui::Key::Enter),
            i.consume_key(n0, egui::Key::Backspace),
            i.consume_key(n0, egui::Key::Home),
            i.consume_key(n0, egui::Key::End),
            i.consume_key(n0, egui::Key::PageUp),
            i.consume_key(n0, egui::Key::PageDown),
        )
    });
    // PgUp/PgDn/Home/End도 목록 선택을 이동(#6). PAGE=한 번에 12칸.
    if !vis.is_empty() && (down || up || home || end || pgup || pgdn) {
        let (n, page) = (vis.len(), 12usize);
        let idx = match cur {
            _ if home => 0,
            _ if end => n - 1,
            Some(i) if pgdn => (i + page).min(n - 1),
            Some(i) if pgup => i.saturating_sub(page),
            Some(i) if down => (i + 1).min(n - 1),
            Some(i) => i.saturating_sub(1),
            None if down || pgdn => 0,
            None => n - 1,
        };
        *selected = Some(vis[idx].name.clone());
        *scroll = true;
    }
    if enter {
        if let Some(r) = cur.map(|i| vis[i]) {
            if r.is_dir {
                return Some(path.join(&r.name));
            }
            crate::paneurl::os_open(&path.join(&r.name).to_string_lossy());
        }
    }
    if back {
        if let Some(parent) = path.parent() {
            return Some(parent.to_path_buf());
        }
    }
    None
}

/// 새 폴더/파일 입력 행 + 이름변경 입력 행을 그리고 확정/취소 플래그를 돌려준다.
/// 반환: (mkdir_ok, mkdir_cancel, rename_ok, rename_cancel).
pub(crate) fn input_rows(
    ui: &mut egui::Ui,
    mkdir: &mut Option<String>,
    rename: &mut Option<(String, String)>,
    new_is_file: bool,
    lang: Lang,
    show_rename: bool,
) -> (bool, bool, bool, bool) {
    let (mut mk_ok, mut mk_cancel, mut rn_ok, mut rn_cancel) = (false, false, false, false);
    if let Some(name) = mkdir.as_mut() {
        let lbl = if new_is_file { "sftp.newfile" } else { "sftp.newfolder" };
        ui.horizontal(|ui| {
            ui.label(tr(lang, lbl));
            ui.text_edit_singleline(name);
            if ui.small_button("\u{2713}").clicked() {
                mk_ok = true;
            }
            if ui.small_button("\u{2715}").clicked() {
                mk_cancel = true;
            }
        });
    }
    // 자세히 보기는 셀에서 인라인 편집(show_rename=false) — 상단 입력 행 생략.
    if let (true, Some((_, newname))) = (show_rename, rename.as_mut()) {
        ui.horizontal(|ui| {
            ui.label(tr(lang, "sftp.renameto"));
            let r = ui.text_edit_singleline(newname);
            let ent = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.small_button("\u{2713}").clicked() || ent {
                rn_ok = true;
            }
            if ui.small_button("\u{2715}").clicked() {
                rn_cancel = true;
            }
        });
    }
    (mk_ok, mk_cancel, rn_ok, rn_cancel)
}
