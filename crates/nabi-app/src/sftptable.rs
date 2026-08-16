//! SFTP 탐색기식 컬럼 테이블(이름·유형·크기·수정일) — 로컬 브라우저와 동일한 모양.

use crate::sftpentries::EClick;
use crate::sftpentryfmt::{cmp_color, cmp_status};
use crate::sftppanel::SftpPanel;
use crate::sftptab::SftpAct;
use crate::sftpview::{actions, icon, RemoteName};
use nabi_i18n::{tr, Lang};
use nabi_proto::SftpEntry;
use std::collections::HashMap;

/// 화면에 실제로 보이는 항목의 이름(필터·숨김 반영).
///
/// 전체 선택·범위 선택·일괄 경로 복사는 **반드시** 이 목록을 쓴다. `sftp.entries`를 직접 쓰면
/// 필터로 가려진 파일까지 선택되어, 이어지는 삭제가 안 보이던 파일을 지운다.
pub(crate) fn visible_names(sftp: &SftpPanel) -> Vec<String> {
    visible(sftp).iter().map(|e| e.name.clone()).collect()
}

/// 필터·숨김 적용한 보이는 항목(테이블/키보드 공용 순서).
fn visible(sftp: &SftpPanel) -> Vec<&SftpEntry> {
    sftp.entries
        .iter()
        .filter(|e| {
            (sftp.show_hidden || !e.name.starts_with('.'))
                && crate::browserfs::name_matches(&sftp.filter, &e.name)
        })
        .collect()
}

/// 키보드 탐색: F5=새로고침, Backspace=상위, ↑↓=선택 이동, Enter=열기/진입. 입력칸 포커스 시 무시.
pub(crate) fn keyboard_nav(ui: &egui::Ui, sftp: &mut SftpPanel, a: &mut SftpAct) {
    // 마우스 엄지 버튼(이전/다음) — 패널 위에서만(포커스와 무관).
    if a.over {
        (a.back, a.fwd) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Extra1),
                i.pointer.button_pressed(egui::PointerButton::Extra2),
            )
        });
    }
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return;
    }
    let (f5, back, up, down, enter, home, end, pgup, pgdn, f2, del) = ui.input(|i| {
        use egui::Key as K;
        (i.key_pressed(K::F5), i.key_pressed(K::Backspace), i.key_pressed(K::ArrowUp), i.key_pressed(K::ArrowDown), i.key_pressed(K::Enter),
         i.key_pressed(K::Home), i.key_pressed(K::End), i.key_pressed(K::PageUp), i.key_pressed(K::PageDown), i.key_pressed(K::F2), i.key_pressed(K::Delete))
    });
    if f5 { a.go = Some(sftp.path.clone()); }
    if back { a.go = Some(crate::sftppath::parent_dir(&sftp.path)); }
    // 표준 파일관리 단축키: F2=이름변경, Delete=삭제(선택 항목). 기존 핸들러로 위임.
    if f2 && sftp.rename_from.is_none() { a.rename = a.rename.take().or_else(|| sftp.selected.clone()); }
    if del { a.del = a.del.take().or_else(|| sftp.selected.clone()); }
    // Ctrl+A: 보이는 항목만 선택(필터로 가려진 파일이 삭제에 휩쓸리지 않게).
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) { sftp.multi = visible_names(sftp).into_iter().collect(); }
    let nav = up || down || home || end || pgup || pgdn;
    if !(nav || enter) {
        return;
    }
    let (mut new_sel, mut open) = (None, None);
    {
        let vis = visible(sftp);
        let cur = sftp.selected.as_ref().and_then(|s| vis.iter().position(|e| &e.name == s));
        if nav && !vis.is_empty() {
            let (n, page) = (vis.len(), 12usize);
            let idx = match cur {
                _ if home => 0, _ if end => n - 1,
                Some(i) if pgdn => (i + page).min(n - 1),
                Some(i) if pgup => i.saturating_sub(page),
                Some(i) if down => (i + 1).min(n - 1),
                Some(i) => i.saturating_sub(1),
                None if down || pgdn => 0,
                None => n - 1,
            };
            new_sel = Some(vis[idx].name.clone());
        }
        if enter {
            if let Some(e) = cur.map(|i| vis[i]) {
                open = Some((e.name.clone(), e.is_dir, e.size));
            }
        }
    }
    if let Some(s) = new_sel {
        sftp.selected = Some(s);
        sftp.scroll = true;
    }
    if let Some((name, is_dir, size)) = open {
        if is_dir {
            a.go = Some(crate::sftppath::join_path(&sftp.path, &name));
        } else {
            a.dl = Some((name, size));
        }
    }
}

// perms_menu는 sftpperms.rs로 분리(파일 크기 규율).

/// 파일 유형 라벨(폴더 또는 확장자 대문자).
fn type_label(e: &SftpEntry, lang: Lang) -> String {
    if e.is_dir {
        return tr(lang, "browser.type.folder").to_string();
    }
    std::path::Path::new(&e.name)
        .extension()
        .map(|x| x.to_string_lossy().to_uppercase())
        .unwrap_or_default()
}

/// 이름 셀: 색칠된 아이콘+이름(click_and_drag — 더블클릭 열기, 드래그 다운로드 페이로드).
/// 이 항목이 이름변경 대상이면 셀 위치에 인라인 편집기를 그린다(탐색기식).
fn name_cell(
    ui: &mut egui::Ui,
    e: &SftpEntry,
    cur: &str,
    lang: Lang,
    compare: Option<&HashMap<String, (bool, u64)>>,
    ren: &mut crate::renameui::RenameUi,
) -> Option<EClick> {
    let base = if e.is_dir {
        crate::filetype::FOLDER_COLOR
    } else {
        crate::filetype::file_color(&e.name)
    };
    let mut color = base;
    if let Some(map) = compare {
        if let Some(c) = cmp_color(cmp_status(&e.name, e.size, e.is_dir, map)) {
            color = c;
        }
    }
    // 칸 너비 전체를 클릭 영역으로 — 라벨은 좌측 정렬로 그림.
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(40.0), ui.available_height().max(16.0)),
        egui::Sense::click_and_drag(),
    );
    if ren.try_edit(ui, rect, &e.name) {
        return None; // 인라인 편집 중 — 라벨/상호작용 생략.
    }
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        format!("{} {}", icon(e), e.name),
        egui::TextStyle::Body.resolve(ui.style()),
        color,
    );
    if resp.dragged() {
        resp.dnd_set_drag_payload(RemoteName {
            name: e.name.clone(),
            is_dir: e.is_dir,
        });
    }
    // 로컬 파일을 끌어 폴더 위에 올리면 테두리로 강조(업로드 대상 안내).
    if e.is_dir && resp.dnd_hover_payload::<String>().is_some() {
        let c = ui.visuals().selection.stroke.color;
        ui.painter().rect_stroke(resp.rect, 3.0, egui::Stroke::new(2.0, c), egui::StrokeKind::Inside);
    }
    let resp = if e.mode & 0o777 != 0 {
        let rwx = crate::sftpentryfmt::mode_to_rwx(e.mode, e.is_dir, e.is_link);
        resp.on_hover_text(format!("{rwx}  ({:03o})", e.mode & 0o777))
    } else {
        resp
    };
    actions(&resp, e, cur, lang)
}

/// 탐색기식 컬럼 테이블. 헤더 클릭=정렬(같은 컬럼 재클릭=방향 토글은 처리측).
#[allow(clippy::too_many_arguments)]
pub(crate) fn table(
    ui: &mut egui::Ui,
    entries: &[&SftpEntry],
    cur: &str,
    lang: Lang,
    compare: Option<&HashMap<String, (bool, u64)>>,
    selected: Option<&str>,
    multi: &std::collections::HashSet<String>,
    scroll_to: bool,
    ren: &mut crate::renameui::RenameUi,
) -> Option<EClick> {
    use crate::browserfs::Sort;
    use egui_extras::{Column, TableBuilder};
    let mut click: Option<EClick> = None;
    let mut set_sort: Option<Sort> = None;
    let hdr = |ui: &mut egui::Ui, label: &str, s: Sort, set: &mut Option<Sort>| {
        if ui
            .add(egui::Label::new(egui::RichText::new(label).strong()).sense(egui::Sense::click()))
            .clicked()
        {
            *set = Some(s);
        }
    };
    let rh = crate::browsercols::row_h(ui); // 글꼴 크기에 따른 행 높이(Ctrl+휠 줌).
    let mut tb = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false]) // 남은 높이까지 채움(아래 빈 영역 포함).
        .column(Column::initial(150.0).at_least(80.0).clip(true)) // 이름
        .column(Column::initial(56.0).at_least(40.0)) // 유형
        .column(Column::initial(72.0).at_least(50.0)) // 크기
        .column(Column::initial(150.0).at_least(120.0)) // 수정일 — 리사이즈/자동맞춤 가능(브라우저와 동일 구조, #11)
        .column(Column::remainder()); // 빈 필러
    if scroll_to {
        if let Some(idx) = selected.and_then(|s| entries.iter().position(|e| e.name == s)) {
            tb = tb.scroll_to_row(idx, Some(egui::Align::Center));
        }
    }
    tb.header(rh, |mut h| {
            h.col(|ui| hdr(ui, tr(lang, "browser.col.name"), Sort::Name, &mut set_sort));
            h.col(|ui| hdr(ui, tr(lang, "browser.col.type"), Sort::Type, &mut set_sort));
            h.col(|ui| hdr(ui, tr(lang, "browser.col.size"), Sort::Size, &mut set_sort));
            h.col(|ui| hdr(ui, tr(lang, "browser.col.modified"), Sort::Date, &mut set_sort));
        })
        .body(|body| {
            // body.rows()는 보이는 행만 그린다. row()를 항목마다 부르면 화면 밖 수천 행까지
            // 매 프레임 레이아웃·페인트한다(대용량 디렉터리에서 그대로 프레임 시간이 된다).
            let up = cur != "/" && cur != "."; // 맨 위 ".." 행(상위 이동) 유무.
            let total = entries.len() + usize::from(up);
            body.rows(rh, total, |mut r| {
                let idx = r.index();
                if up && idx == 0 {
                    r.col(|ui| {
                        if crate::browsergrid::up_row(ui) {
                            click = Some(EClick::Nav(crate::sftppath::parent_dir(cur)));
                        }
                    });
                    r.col(|_| {});
                    r.col(|_| {});
                    r.col(|_| {});
                    return;
                }
                let e = entries[idx - usize::from(up)];
                r.set_selected(multi.contains(e.name.as_str()) || selected == Some(e.name.as_str()));
                r.col(|ui| {
                    if let Some(a) = name_cell(ui, e, cur, lang, compare, ren) {
                        click = Some(a);
                    }
                });
                r.col(|ui| {
                    ui.label(type_label(e, lang));
                });
                r.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(if e.is_dir { String::new() } else { crate::browserfs::human(e.size) });
                    });
                });
                r.col(|ui| {
                    ui.label(crate::browserfs::human_datetime(e.mtime));
                });
            });
        });
    click.or(set_sort.map(EClick::SetSort))
}
