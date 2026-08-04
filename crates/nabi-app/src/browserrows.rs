//! 로컬 브라우저 — Windows 탐색기식 컬럼 테이블(이름·유형·크기·수정일). 헤더 클릭=정렬.

use crate::browserfs::{human, human_datetime, Row, Sort};
use crate::sftpview::RemoteName;
use egui_extras::{Column, TableBuilder};
use nabi_i18n::{tr, Lang};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 목록에서 발생한 동작(browser.rs가 적용).
#[derive(Default)]
pub(crate) struct RowActs {
    pub nav: Option<PathBuf>,
    pub delete: Option<String>,
    pub rename: Option<String>,
    pub upload: Option<String>,
    /// (로컬 하위폴더, 원격 항목) — 원격을 이 폴더로 다운로드(폴더 행 드롭).
    pub dl_into: Option<(String, RemoteName)>,
    /// 재귀 크기 계산을 요청한 폴더 이름.
    pub calc_size: Option<String>,
    /// 복제를 요청한 항목 이름.
    pub duplicate: Option<String>,
    /// 컬럼 헤더 클릭으로 선택한 정렬 기준.
    pub set_sort: Option<Sort>,
    /// 단일 클릭 선택: (이름, ctrl, shift).
    pub select: Option<(String, bool, bool)>,
    /// 클립보드로 복사할 항목(탐색기에서 붙여넣기 가능 — CF_HDROP).
    pub copy: Option<String>,
    /// OS 드래그-아웃을 시작할 항목(탐색기로 끌어 놓으면 복사).
    pub os_drag: Option<String>,
    /// 편집을 요청한 파일 이름(내장/외부 에디터).
    pub edit: Option<String>,
    /// HEX(이진) 편집기로 강제로 열 파일 이름.
    pub edit_hex: Option<String>,
    /// 빠른 미리보기 요청 파일 이름(E9).
    pub preview: Option<String>,
}

use crate::browsercols::{header_cell, type_label};

/// 유형/비교 색(비교 모드면 비교색 우선, 아니면 폴더=금색·파일=카테고리색).
pub(crate) fn row_color(row: &Row, remote_map: &HashMap<String, (bool, u64)>) -> egui::Color32 {
    if !remote_map.is_empty() {
        let st = crate::sftpentries::cmp_status(&row.name, row.size, row.is_dir, remote_map);
        if let Some(c) = crate::sftpentries::cmp_color(st) {
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
            ui.painter().rect_stroke(resp.rect, 3.0, egui::Stroke::new(2.0, c));
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
            ui.close_menu();
        }
        if !row.is_dir && ui.button(tr(lang, "nabipad.openhex")).clicked() { acts.edit_hex = Some(row.name.clone()); ui.close_menu(); }
        if !row.is_dir && ui.button(tr(lang, "browser.preview")).clicked() { acts.preview = Some(row.name.clone()); ui.close_menu(); } // E9
        // CF_HDROP 복사 — 탐색기에서 Ctrl+V로 실제 파일 붙여넣기.
        if ui.button(tr(lang, "browser.copy")).clicked() {
            acts.copy = Some(row.name.clone());
            ui.close_menu();
        }
        if ui.button(tr(lang, "browser.copypath")).clicked() {
            ui.ctx().copy_text(full.to_string_lossy().into_owned());
            ui.close_menu();
        }
        if ui.button(tr(lang, "browser.reveal")).clicked() {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", full.display()))
                .spawn();
            ui.close_menu();
        }
        if row.is_dir && ui.button(tr(lang, "browser.calcsize")).clicked() {
            acts.calc_size = Some(row.name.clone());
            ui.close_menu();
        }
        if ui.button(tr(lang, "browser.duplicate")).clicked() {
            acts.duplicate = Some(row.name.clone());
            ui.close_menu();
        }
        if ui.button(tr(lang, "sftp.rename")).clicked() {
            acts.rename = Some(row.name.clone());
            ui.close_menu();
        }
        if ui.button(tr(lang, "browser.delete")).clicked() {
            acts.delete = Some(row.name.clone());
            ui.close_menu();
        }
        if can_upload && !row.is_dir && ui.button(tr(lang, "browser.upload")).clicked() {
            acts.upload = Some(row.name.clone());
            ui.close_menu();
        }
    });
}

/// 이름 컬럼 셀(테이블): 색칠된 아이콘+이름 + 공유 상호작용.
/// 이 행이 이름변경 대상이면 셀 위치에 인라인 편집기를 그린다(탐색기식).
#[allow(clippy::too_many_arguments)]
fn name_cell(
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

/// 탐색기식 컬럼 테이블로 항목을 그린다.
#[allow(clippy::too_many_arguments)]
pub(crate) fn browser_rows(
    ui: &mut egui::Ui,
    entries: &[Row],
    path: &Path,
    filt: &str,
    remote_map: &HashMap<String, (bool, u64)>,
    can_upload: bool,
    lang: Lang,
    sort: Sort,
    desc: bool,
    view: crate::sftpview::ViewMode,
    selected: Option<&str>,
    multi: &std::collections::HashSet<String>,
    scroll_to_selected: bool,
    ren: &mut crate::renameui::RenameUi,
) -> RowActs {
    let mut acts = RowActs::default();
    let visible: Vec<&Row> = entries
        .iter()
        .filter(|r| crate::browserfs::name_matches(filt, &r.name))
        .collect();
    // 빈 폴더/검색 결과 없음 — '..'(상위 이동)는 그대로 제공.
    if visible.is_empty() {
        if let Some(parent) = path.parent() {
            if crate::browsergrid::up_row(ui) {
                acts.nav = Some(parent.to_path_buf());
            }
        }
        crate::browsergrid::empty_message(ui, lang, filt.is_empty());
        return acts;
    }
    let is_sel = |n: &str| multi.contains(n) || selected == Some(n);
    // 자세히(Details)는 컬럼 테이블, 그 외(목록/큰·작은 아이콘/타일)는 아이콘 격자.
    if view != crate::sftpview::ViewMode::Details {
        crate::browsergrid::grid(ui, &visible, path, remote_map, can_upload, lang, view, selected, multi, &mut acts, ren);
        return acts;
    }
    let mut set_sort: Option<Sort> = None;
    let act = |s: Sort| sort == s; // 활성 정렬 컬럼인가.
    let rh = crate::browsercols::row_h(ui); // 글꼴 크기에 따른 행 높이(Ctrl+휠 줌).
    let mut tb = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false]) // 남은 높이까지 채움(아래 빈 영역도 패널의 일부로).
        .column(Column::initial(150.0).at_least(80.0).clip(true)) // 이름
        .column(Column::initial(56.0).at_least(40.0)) // 유형
        .column(Column::initial(72.0).at_least(50.0)) // 크기
        .column(Column::initial(150.0).at_least(120.0)) // 수정일 — clip 제거로 더블클릭 자동맞춤이 내용폭 측정(#11)
        .column(Column::remainder()); // 빈 필러 — 수정일이 남은 공간을 다 차지하지 않게
    // 키보드 이동 시 선택 행이 보이도록 스크롤.
    if scroll_to_selected {
        if let Some(idx) = selected.and_then(|s| visible.iter().position(|r| r.name == s)) {
            tb = tb.scroll_to_row(idx, Some(egui::Align::Center));
        }
    }
    tb.header(rh, |mut h| {
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.name"), Some(Sort::Name), act(Sort::Name), desc, &mut set_sort));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.type"), Some(Sort::Type), act(Sort::Type), desc, &mut set_sort));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.size"), Some(Sort::Size), act(Sort::Size), desc, &mut set_sort));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.modified"), Some(Sort::Date), act(Sort::Date), desc, &mut set_sort));
            h.col(|_| {}); // 빈 필러 헤더.
        })
        .body(|mut body| {
            // 맨 위 ".." 행: 더블클릭으로 상위 폴더 이동(루트가 아닐 때).
            if let Some(parent) = path.parent().map(|p| p.to_path_buf()) {
                body.row(rh, |mut r| {
                    r.col(|ui| {
                        if crate::browsergrid::up_row(ui) {
                            acts.nav = Some(parent.clone());
                        }
                    });
                    r.col(|_| {});
                    r.col(|_| {});
                    r.col(|_| {});
                    r.col(|_| {});
                });
            }
            for row in &visible {
                body.row(rh, |mut r| {
                    let sel = is_sel(row.name.as_str());
                    r.set_selected(sel); // 선택(다중 포함) 하이라이트.
                    r.col(|ui| name_cell(ui, row, path, remote_map, can_upload, lang, sel, &mut acts, ren));
                    r.col(|ui| {
                        ui.label(type_label(row, lang));
                    });
                    r.col(|ui| {
                        // 크기는 오른쪽 정렬(탐색기식).
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(if row.is_dir { String::new() } else { human(row.size) });
                        });
                    });
                    r.col(|ui| {
                        ui.label(human_datetime(row.mtime));
                    });
                    r.col(|_| {}); // 빈 필러 셀.
                });
            }
        });
    acts.set_sort = set_sort;
    acts
}
