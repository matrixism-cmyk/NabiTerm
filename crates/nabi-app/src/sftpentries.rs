//! SFTP 항목 목록 렌더 + 클릭 동작 — show_sftp에서 분리(라인 한도).

use crate::sftppanel::SftpPanel;
use nabi_i18n::{tr, Lang};
use nabi_proto::SftpEntry;

/// 일괄 이름변경: name에서 find→replace 치환한 새 이름(바뀌지 않으면 None).
/// replace 토큰: `{n}`=순번, `{nn}`/`{nnn}`=0채움 순번, `{name}`=확장자 뺀 원본명, `{ext}`=확장자.
pub(crate) fn batch_new_name(name: &str, find: &str, replace: &str, idx: usize) -> Option<String> {
    if find.is_empty() || !name.contains(find) {
        return None;
    }
    // 확장자 분리(숨김파일 .bashrc는 통째로 name 취급).
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b, e),
        _ => (name, ""),
    };
    let replace = replace
        .replace("{nnn}", &format!("{idx:03}"))
        .replace("{nn}", &format!("{idx:02}"))
        .replace("{n}", &idx.to_string())
        .replace("{name}", base)
        .replace("{ext}", ext);
    let new = name.replace(find, &replace);
    (new != name && !new.is_empty()).then_some(new)
}


/// 원격 항목을 폴더 우선 + 기준(이름/크기/날짜)으로 정렬.
pub(crate) fn sort_sftp(entries: &mut [SftpEntry], sort: crate::browserfs::Sort, desc: bool) {
    use crate::browserfs::Sort;
    let ext = |n: &str| {
        std::path::Path::new(n)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            let ord = match sort {
                Sort::Size => a.size.cmp(&b.size),
                Sort::Date => a.mtime.cmp(&b.mtime),
                Sort::Type => ext(&a.name).cmp(&ext(&b.name)).then_with(|| crate::browserfs::natural_cmp(&a.name, &b.name)),
                Sort::Name => crate::browserfs::natural_cmp(&a.name, &b.name),
            };
            if desc {
                ord.reverse()
            } else {
                ord
            }
        })
    });
}

/// 이름변경/새폴더/삭제확인 입력 행에서 발생한 동작.
pub(crate) enum RowAction {
    None,
    Rename,
    Mkdir,
    DelOk,
    DelCancel,
}

/// 이름 변경·새 폴더 입력 행 + 삭제 확인 행을 그리고 동작을 돌려준다.
pub(crate) fn input_rows(
    ui: &mut egui::Ui,
    panel: &mut SftpPanel,
    lang: Lang,
    details: bool,
) -> RowAction {
    let mut act = RowAction::None;
    // 자세히 보기는 셀에서 인라인 편집 — 상단 이름변경 행 생략(mkdir/삭제 행은 유지).
    let renaming = panel.rename_from.is_some() && !details;
    if renaming || panel.mkdir_mode {
        let lbl = if renaming {
            "sftp.renameto"
        } else if panel.newfile_mode {
            "sftp.newfile"
        } else {
            "sftp.newfolder"
        };
        ui.horizontal(|ui| {
            ui.label(tr(lang, lbl));
            let resp = ui.text_edit_singleline(&mut panel.input);
            // Enter(필드에서)로도 확정.
            let entered = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.small_button("\u{2713}").clicked() || entered {
                act = if renaming { RowAction::Rename } else { RowAction::Mkdir };
            }
            if ui.small_button("\u{2715}").clicked() {
                // 취소: 입력 모드 해제(여기서 직접 처리).
                panel.rename_from = None;
                panel.mkdir_mode = false;
                panel.newfile_mode = false;
                panel.input.clear();
            }
        });
    }
    if let Some(name) = panel.pending_delete.clone() {
        // 폴더는 재귀 삭제이므로 하위 전체 삭제임을 경고(데이터 손실 방지). 다중 선택이면 개수도.
        let is_dir = panel.entries.iter().any(|e| e.name == name && e.is_dir);
        let n = panel.multi.len();
        let msg = if n > 1 && panel.multi.contains(&name) {
            format!("{} ({}\u{2713})?", tr(lang, "sftp.deletemulti"), n)
        } else if is_dir {
            format!("{} · {name}?", tr(lang, "sftp.deletedir"))
        } else {
            format!("{} {name}?", tr(lang, "sftp.delete"))
        };
        ui.horizontal(|ui| {
            ui.colored_label(crate::theme_ui::ERR, msg);
            if ui.small_button("\u{2713}").clicked() {
                act = RowAction::DelOk;
            }
            if ui.small_button("\u{2715}").clicked() {
                act = RowAction::DelCancel;
            }
        });
    }
    act
}

/// 항목에서 발생한 사용자 동작.
pub(crate) enum EClick {
    Nav(String),
    Download(String, u64),
    DownloadDir(String),
    /// 폴더 크기 재귀 계산.
    DirSize(String),
    /// 이 폴더에서 SSH 터미널 열기(빠른연결 프리필).
    OpenTermHere(String),
    Edit(String),
    Chmod(String, u32),
    /// 권한 재귀 적용(하위 전부).
    ChmodRecursive(String, u32),
    Rename(String),
    Delete(String),
    /// 로컬 경로를 이 (폴더, 로컬경로)로 업로드(폴더 행 드롭).
    DropInto(String, String),
    /// 컬럼 헤더 클릭으로 선택한 정렬 기준(같은 컬럼 재클릭=방향 토글은 처리측에서).
    SetSort(crate::browserfs::Sort),
    /// 단일 클릭 선택: (이름, ctrl, shift) — 토글/범위는 처리측.
    Select(String, bool, bool),
    /// 원격 파일/폴더를 탐색기로 드래그-아웃: (이름, 크기, 폴더 여부).
    OsDrag(String, u64, bool),
}

/// 원격 항목 목록(폴더=네비, 파일=다운로드, 우클릭=폴더받기/이름변경/삭제)을 그린다.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_entries(
    ui: &mut egui::Ui,
    entries: &[SftpEntry],
    cur_path: &str,
    lang: Lang,
    filter: &str,
    compare: Option<&std::collections::HashMap<String, (bool, u64)>>,
    show_hidden: bool,
    mode: crate::sftpview::ViewMode,
    selected: Option<&str>,
    multi: &std::collections::HashSet<String>,
    scroll_to: bool,
    ren: &mut crate::renameui::RenameUi,
) -> Option<EClick> {
    let now = std::time::UNIX_EPOCH.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    let visible: Vec<&SftpEntry> = entries
        .iter()
        .filter(|e| {
            (show_hidden || !e.name.starts_with('.'))
                && crate::browserfs::name_matches(filter, &e.name)
        })
        .collect();
    // 빈 폴더/검색 결과 없음 — '..'(상위 이동)는 그대로 제공.
    if visible.is_empty() {
        let mut click = None;
        if cur_path != "/" && cur_path != "." && crate::browsergrid::up_row(ui) {
            click = Some(EClick::Nav(crate::sftppath::parent_dir(cur_path)));
        }
        crate::browsergrid::empty_message(ui, lang, filter.is_empty());
        return click;
    }
    // 자세히(Details)는 탐색기식 컬럼 테이블(자체 스크롤). 그 외 모드는 격자/내용을 스크롤 영역에.
    if matches!(mode, crate::sftpview::ViewMode::Details) {
        crate::sftptable::table(ui, &visible, cur_path, lang, compare, selected, multi, scroll_to, ren)
    } else {
        egui::ScrollArea::vertical()
            .id_salt("sftp_entries")
            .show(ui, |ui| {
                crate::sftpview::render(ui, &visible, cur_path, lang, mode, now, compare, selected)
            })
            .inner
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::batch_new_name;

    #[test]
    fn batch_rename_replaces() {
        assert_eq!(batch_new_name("a.txt", ".txt", ".bak", 1).as_deref(), Some("a.bak"));
        assert_eq!(batch_new_name("b.log", ".txt", ".bak", 1), None);
        assert_eq!(batch_new_name("x", "", "y", 1), None);
        assert_eq!(batch_new_name("img1", "img", "photo", 1).as_deref(), Some("photo1"));
        // {n} 순번 치환.
        assert_eq!(batch_new_name("draft", "draft", "v{n}", 3).as_deref(), Some("v3"));
        assert_eq!(batch_new_name("a_x", "x", "{n}", 7).as_deref(), Some("a_7"));
        // 0채움 순번 {nn}/{nnn} + {name}/{ext} 토큰(find는 한 번만 나오는 부분 사용).
        assert_eq!(batch_new_name("photo.png", "photo", "{nnn}", 5).as_deref(), Some("005.png"));
        assert_eq!(batch_new_name("photo.png", "photo", "{name}_{nn}", 5).as_deref(), Some("photo_05.png"));
        assert_eq!(batch_new_name("photo.png", "photo", "{name}-{ext}", 1).as_deref(), Some("photo-png.png"));
    }
}
