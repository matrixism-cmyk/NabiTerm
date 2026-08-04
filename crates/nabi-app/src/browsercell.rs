//! 브라우저 목록의 이름 칸 그리기(아이콘·이름 편집·드래그). 행 상호작용은 browserrows.

use crate::browserfs::Row;
use crate::browserrows::{row_color, row_interact, RowActs};
use nabi_i18n::Lang;
use std::collections::HashMap;
use std::path::Path;
/// 이 행이 이름변경 대상이면 셀 위치에 인라인 편집기를 그린다(탐색기식).
#[allow(clippy::too_many_arguments)]
pub(crate) fn name_cell(
    ui: &mut egui::Ui,
    row: &Row,
    path: &Path,
    remote_map: &HashMap<String, (bool, u64)>,
    can_upload: bool,
    lang: Lang,
    is_selected: bool,
    acts: &mut RowActs,
    ren: &mut crate::renameui::RenameUi,
) {
    let icon = if row.is_link {
        "\u{1f517}".to_string() // 🔗 심볼릭 링크(SFTP 표시와 일관). 더블클릭은 대상이 dir면 진입.
    } else if row.is_dir {
        "\u{1f4c1}".to_string()
    } else {
        crate::filetype::file_icon(&row.name).to_string()
    };
    let text = format!("{icon} {}", row.name);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = row_color(row, remote_map);
    // 보통은 칸 전체를 클릭 영역으로(텍스트만 노리지 않아도 됨). 단, 자동맞춤(sizing pass) 때는
    // 자연 너비(아이콘+이름)를 보고해야 구분선 더블클릭이 칸을 내용 최대값에 맞춘다.
    let w = if ui.is_sizing_pass() {
        ui.painter().layout_no_wrap(text.clone(), font.clone(), color).size().x + 6.0
    } else {
        ui.available_width().max(40.0)
    };
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(w, ui.available_height().max(16.0)), egui::Sense::click_and_drag());
    if ren.try_edit(ui, rect, &row.name) {
        return; // 인라인 편집 중 — 라벨/상호작용 생략.
    }
    ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, text, font, color);
    row_interact(ui, &resp, row, path, can_upload, lang, is_selected, acts);
}

