//! 로컬 브라우저 테이블의 컬럼 헬퍼(유형 라벨·정렬 헤더) — browserrows에서 분리(라인 한도).

use crate::browserfs::{Row, Sort};
use nabi_i18n::{tr, Lang};
use std::path::Path;

/// 파일 목록 영역의 글꼴 크기를 적용한다(Body·Button 텍스트 스타일 덮어쓰기).
/// 이 ui와 그 자식에만 적용되므로 브라우저 영역만 스케일된다(전역 영향 없음).
pub(crate) fn apply_list_font(ui: &mut egui::Ui, size: f32) {
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        if let Some(f) = ui.style_mut().text_styles.get_mut(&style) {
            f.size = size;
        }
    }
}

/// 현재 Body 글꼴 높이 기반의 테이블 행 높이(글꼴 크기에 따라 행도 커짐).
pub(crate) fn row_h(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body).max(14.0) + 6.0
}

/// 파일 유형 라벨(폴더 또는 확장자 대문자) — 이름 기반 공용(#12: 브라우저·SFTP 공유).
pub(crate) fn type_label_of(name: &str, is_dir: bool, lang: Lang) -> String {
    if is_dir {
        return tr(lang, "browser.type.folder").to_string();
    }
    Path::new(name).extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default()
}

/// 파일 유형 라벨(로컬 브라우저 Row).
pub(crate) fn type_label(row: &Row, lang: Lang) -> String {
    type_label_of(&row.name, row.is_dir, lang)
}

/// 정렬 가능한 헤더 셀: 텍스트는 가운데 정렬, 칸 전체가 클릭 영역(텍스트만 노리지 않아도 됨).
/// 우측 여백을 비워 컬럼 리사이즈 핸들(더블클릭=자동맞춤) 입력을 가리지 않는다.
pub(crate) fn header_cell(
    ui: &mut egui::Ui,
    label: &str,
    sort: Option<Sort>,
    active: bool,
    desc: bool,
    set_sort: &mut Option<Sort>,
) {
    let arrow = if active {
        if desc { " \u{25be}" } else { " \u{25b4}" }
    } else {
        ""
    };
    let w = ui.available_width().max(20.0);
    let h = ui.available_height().max(16.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    // 리사이즈 핸들(우측 ~8px)은 클릭 영역에서 제외 — 그래야 구분선 드래그/더블클릭이 동작.
    let click = egui::Rect::from_min_max(rect.min, egui::pos2((rect.max.x - 8.0).max(rect.min.x), rect.max.y));
    let resp = ui.interact(click, ui.id().with(("hdr", label)), egui::Sense::click());
    // **머리글은 그 칸 자료와 같은 쪽에 붙인다.** 가운데 정렬로 두었더니 이름 칸이 넓어진
    // 뒤 "이름" 이 칸 오른쪽 끝에 떠서 어느 칸의 머리글인지 알아보기 어려웠다
    // (2026-08-31 화면으로 확인). 크기만 오른쪽 — 자료가 오른쪽 정렬이다.
    let right = matches!(sort, Some(Sort::Size));
    let (at, align) = match right {
        true => (egui::pos2(rect.max.x - 6.0, rect.center().y), egui::Align2::RIGHT_CENTER),
        false => (egui::pos2(rect.min.x + 6.0, rect.center().y), egui::Align2::LEFT_CENTER),
    };
    ui.painter().text(
        at,
        align,
        format!("{label}{arrow}"),
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
    if let (Some(s), true) = (sort, resp.clicked()) {
        *set_sort = Some(s);
    }
}
