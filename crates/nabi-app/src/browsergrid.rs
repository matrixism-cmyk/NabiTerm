//! 로컬 브라우저 아이콘 격자(목록/큰·작은 아이콘/타일) — 자세히(테이블) 외 보기 모드.

use crate::browserfs::{human, Row};
use crate::browserrows::{row_color, row_interact, RowActs};
use crate::sftpview::ViewMode;
use nabi_i18n::Lang;
use std::collections::HashMap;
use std::path::Path;

/// 전폭 '..'(상위 이동) 행을 그리고 더블클릭됐는지 돌려준다(로컬·SFTP 공용).
pub(crate) fn up_row(ui: &mut egui::Ui) -> bool {
    up_cell(ui, true)
}

/// 맨 위 ".." 행의 **한 칸**. 두 번 누르면 true(상위 폴더로 간다).
///
/// `label` 이면 화살표와 `..` 를 그리고, 아니면 글자 없이 자리만 차지한다. 글자가 없어도
/// **누르는 넓이는 같다** — 그것이 이 함수가 나뉜 이유다.
///
/// ## 왜 손으로 그리는가
///
/// 예전에는 `add_sized` 를 썼는데, 그 함수는 위젯을 **가운데에 놓는다**
/// (`centered_and_justified` 를 강제한다). 그래서 `..` 만 파일 목록 한가운데에 떠 있어
/// 어색했다 — 아래 줄들은 전부 왼쪽에서 시작하는데 이 줄만 달랐다(2026-09-01 사용자 보고).
///
/// 다른 칸들과 같은 자리에서 시작하려면 자리를 직접 잡고 왼쪽에 그리는 수밖에 없다
/// (이름 칸 `browsercell::name_cell` 도 같은 이유로 그렇게 한다).
pub(crate) fn up_cell(ui: &mut egui::Ui, label: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(16.0), ui.available_height().max(16.0)),
        egui::Sense::click(),
    );
    if label {
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.painter().text(
            rect.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "\u{2b06} ..",
            font,
            ui.visuals().text_color(),
        );
    }
    resp.double_clicked()
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
