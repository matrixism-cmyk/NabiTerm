//! 로컬 브라우저 아이콘 격자(목록/큰·작은 아이콘/타일) — 자세히(테이블) 외 보기 모드.

use crate::browserfs::{human, Row};
use crate::browserrows::{row_color, row_interact, RowActs};
use crate::sftpview::ViewMode;
use nabi_i18n::Lang;
use std::collections::HashMap;
use std::path::Path;

/// 전폭 '..'(상위 이동) 행을 그리고 더블클릭됐는지 돌려준다(로컬·SFTP 공용).
pub(crate) fn up_row(ui: &mut egui::Ui) -> bool {
    ui.add_sized(
        [ui.available_width(), 18.0],
        egui::Label::new("\u{2b06} ..").selectable(false).sense(egui::Sense::click()),
    )
    .double_clicked()
}

/// 빈 폴더/검색 결과 없음 안내(로컬·SFTP 공용).
pub(crate) fn empty_message(ui: &mut egui::Ui, lang: Lang, filter_empty: bool) {
    let key = if filter_empty { "browser.empty" } else { "browser.nomatch" };
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.weak(nabi_i18n::tr(lang, key));
    });
}

/// 보기 모드별 (셀너비, 셀높이, 글자크기, 큰아이콘여부).
fn dims(view: ViewMode) -> (f32, f32, f32, bool) {
    match view {
        ViewMode::List => (150.0, 20.0, 13.0, false),
        ViewMode::LargeIcons => (104.0, 86.0, 34.0, true),
        ViewMode::SmallIcons => (78.0, 56.0, 18.0, true),
        ViewMode::Tile => (180.0, 26.0, 14.0, false),
        _ => (150.0, 20.0, 13.0, false),
    }
}

/// 아이콘 격자를 그린다(자동 줄바꿈). 큰아이콘이면 아이콘 위·이름 아래 2줄.
#[allow(clippy::too_many_arguments)]
pub(crate) fn grid(
    ui: &mut egui::Ui,
    visible: &[&Row],
    path: &Path,
    remote_map: &HashMap<String, (bool, u64)>,
    can_upload: bool,
    lang: Lang,
    view: ViewMode,
    selected: Option<&str>,
    multi: &std::collections::HashSet<String>,
    acts: &mut RowActs,
    ren: &mut crate::renameui::RenameUi,
) {
    let (cw, ch, txt, big) = dims(view);
    egui::ScrollArea::vertical().id_salt("browser_grid").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            // 맨 앞 ".." 셀: 더블클릭으로 상위 폴더 이동.
            if let Some(parent) = path.parent().map(|p| p.to_path_buf()) {
                let up = egui::RichText::new("\u{2b06} ..").size(txt);
                if ui.add_sized([cw, ch], egui::Button::new(up)).double_clicked() {
                    acts.nav = Some(parent);
                }
            }
            for row in visible {
                // 이름변경 중인 항목은 버튼 대신 인라인 편집기를 그 셀에 그린다(아이콘/타일 보기 인라인 rename).
                if ren.active(&row.name) {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(cw, ch), egui::Sense::hover());
                    ren.try_edit(ui, rect, &row.name);
                    continue;
                }
                let icon = if row.is_dir {
                    "\u{1f4c1}".to_string()
                } else {
                    crate::filetype::file_icon(&row.name).to_string()
                };
                let sz = if big && !row.is_dir {
                    format!("\n{}", human(row.size))
                } else {
                    String::new()
                };
                let sep = if big { '\n' } else { ' ' };
                let label = egui::RichText::new(format!("{icon}{sep}{}{sz}", row.name))
                    .size(txt)
                    .color(row_color(row, remote_map));
                let is_sel = multi.contains(row.name.as_str()) || Some(row.name.as_str()) == selected;
                let mut btn = egui::Button::new(label).sense(egui::Sense::click_and_drag());
                if is_sel {
                    btn = btn.fill(ui.visuals().selection.bg_fill); // 선택(다중 포함) 강조.
                }
                let resp = ui.add_sized([cw, ch], btn);
                row_interact(ui, &resp, row, path, can_upload, lang, is_sel, acts);
            }
        });
    });
}
