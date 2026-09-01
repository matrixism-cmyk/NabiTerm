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
    /// 속성 창을 열 대상(파일 또는 폴더 이름).
    pub props: Option<String>,
    /// 복제를 요청한 항목 이름.
    pub duplicate: Option<String>,
    /// zip으로 묶을 항목(고른 것이 있으면 그쪽이 우선).
    pub zip_make: Option<String>,
    /// 풀 zip 파일 이름.
    pub zip_extract: Option<String>,
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
    /// 머리글 오른쪽 클릭으로 켜고 끈 선택 열 이름(`colset`). 표를 다 그린 뒤 적용한다.
    pub toggle_col: Option<&'static str>,
}

use crate::browsercols::{header_cell, type_label};

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
    cols: &[String],
    ren: &mut crate::renameui::RenameUi,
) -> RowActs {
    let mut acts = RowActs::default();
    let visible: Vec<&Row> = entries
        .iter()
        .filter(|r| crate::browserfilter::name_matches(filt, &r.name))
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
        // **이름이 남는 자리를 가져간다.** 예전에는 이름을 150으로 고정하고 맨 끝에 빈
        // 필러를 두었더니, 넓은 창에서 **오른쪽 절반이 통째로 비고 이름은 잘렸다**
        // (2026-08-31 화면으로 확인). 파일 목록에서 가장 길고 가장 중요한 것이 이름이다.
        //
        // 나머지 셋은 내용 폭이 정해져 있어 고정으로 둔다 — 늘어나 봐야 빈칸만 는다.
        .column(Column::remainder().at_least(160.0).clip(true)) // 이름
        .column(Column::initial(56.0).at_least(40.0)) // 유형
        .column(Column::initial(72.0).at_least(50.0)) // 크기
        // 수정일은 clip 을 안 건다 — 구분선 더블클릭 자동맞춤이 내용 폭을 재야 한다(#11).
        .column(Column::initial(150.0).at_least(120.0)); // 수정일
    // 켠 선택 열만, 카탈로그 차례대로. 날짜는 넓고 나머지는 좁다.
    let extra = crate::colset::enabled(&crate::colset::LOCAL, cols);
    for (key, _) in &extra {
        tb = match *key {
            "created" => tb.column(Column::initial(150.0).at_least(120.0)),
            _ => tb.column(Column::initial(64.0).at_least(36.0)),
        };
    }
    // 키보드 이동 시 선택 행이 보이도록 스크롤.
    if scroll_to_selected {
        if let Some(idx) = selected.and_then(|s| visible.iter().position(|r| r.name == s)) {
            tb = tb.scroll_to_row(idx, Some(egui::Align::Center));
        }
    }
    // 머리글 어디를 오른쪽 클릭해도 열 고르기가 뜬다(탐색기와 같다).
    let mut menu = crate::browsercols::ColMenu { cat: &crate::colset::LOCAL, on: cols, toggled: None };
    tb.header(rh, |mut h| {
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.name"), Some(Sort::Name), act(Sort::Name), desc, &mut set_sort, lang, &mut menu));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.type"), Some(Sort::Type), act(Sort::Type), desc, &mut set_sort, lang, &mut menu));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.size"), Some(Sort::Size), act(Sort::Size), desc, &mut set_sort, lang, &mut menu));
            h.col(|ui| header_cell(ui, tr(lang, "browser.col.modified"), Some(Sort::Date), act(Sort::Date), desc, &mut set_sort, lang, &mut menu));
            // 선택 열로는 정렬하지 않는다 — 정렬 기준(`Sort`)에 없고, 넣어 봐야 "R" 끼리
            // 모으는 일이라 쓸 데가 거의 없다. 머리글은 이름표로만 둔다.
            for (_, label) in &extra {
                h.col(|ui| header_cell(ui, tr(lang, label), None, false, desc, &mut set_sort, lang, &mut menu));
            }
        })
        .body(|body| {
            // body.rows()는 보이는 행만 그린다. row()를 항목마다 부르면 화면 밖 수천 행까지
            // 매 프레임 레이아웃·페인트한다(대용량 폴더에서 그대로 프레임 시간이 된다).
            let parent = path.parent().map(|p| p.to_path_buf()); // 맨 위 ".." 행(상위 이동).
            let up = parent.is_some();
            let total = visible.len() + usize::from(up);
            body.rows(rh, total, |mut r| {
                let idx = r.index();
                if let (true, 0) = (up, idx) {
                    // **어느 칸을 두 번 눌러도 올라간다.** 예전에는 이름 칸만 반응해서,
                    // 크기나 날짜 쪽을 눌러 본 사람에게는 고장으로 보였다(2026-09-01 보고).
                    for c in 0..4 + extra.len() {
                        r.col(|ui| {
                            if crate::browsergrid::up_cell(ui, c == 0) {
                                acts.nav = parent.clone();
                            }
                        });
                    }
                    return;
                }
                let row = &visible[idx - usize::from(up)];
                let sel = is_sel(row.name.as_str());
                r.set_selected(sel); // 선택(다중 포함) 하이라이트.
                r.col(|ui| crate::browsercell::name_cell(ui, row, path, remote_map, can_upload, lang, sel, &mut acts, ren));
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
                // 선택 열 — 카탈로그 차례대로, 켠 것만.
                for (key, _) in &extra {
                    r.col(|ui| {
                        ui.label(match *key {
                            "attrs" => crate::browserattr::attr_flags(row.attrs),
                            "created" => human_datetime(row.created),
                            // 확장자는 정렬이 쓰는 그 함수를 그대로 쓴다(소문자·점 없음).
                            // 두 벌로 두면 목록의 확장자와 정렬 기준이 언젠가 갈린다.
                            "ext" => crate::browsersort::ext_of(&row.name),
                            _ => String::new(),
                        });
                    });
                }
            });
        });
    acts.toggle_col = menu.toggled;
    acts.set_sort = set_sort;
    acts
}
