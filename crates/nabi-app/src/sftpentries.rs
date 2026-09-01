//! SFTP 항목 목록 렌더 + 클릭 동작 — show_sftp에서 분리(라인 한도).

use crate::sftppanel::SftpPanel;
use nabi_i18n::{tr, Lang};
use nabi_proto::SftpEntry;

// 일괄 이름변경 규칙은 [`crate::renamerule`] 한 곳에만 있다 — 로컬 창과 원격 창이 같은
// 규칙으로 이름을 만들어야 사용자가 두 화면을 같은 것으로 믿을 수 있다(배치 AJ).
//
// 예전에는 여기서 `batch_new_name` 을 그대로 내보내 원격이 **한 개씩** 이름을 만들었다.
// 그러면 이름 충돌 검사를 아무도 못 한다(그 검사는 목록 전체를 봐야 한다). 이제 원격도
// `renamerule::plan_batch` 를 지나가므로, 이 재수출은 쓰이지 않는다 — 지웠다(2026-09-01).

/// 일괄 이름 바꾸기가 **손댈 것들** — 고른 것이 있으면 그것만, 없으면 이 폴더 전체.
///
/// ## 왜 한 곳에 모으나
///
/// 미리보기의 개수 `(n)` 과 실제로 바꾸는 목록이 **따로 세고 있었다.** 둘이 다른 것을 세면
/// "3개 바뀝니다"라고 보여 주고 스무 개를 바꾸게 된다. 같은 함수가 답하게 한다.
///
/// ## 왜 고른 것을 먼저 보나
///
/// 로컬 창은 **고른 파일만** 바꾼다. 원격만 폴더 전체를 바꾸고 있어서, 같은 규칙을 같은
/// 마음으로 걸어도 결과가 달랐다 — 셋을 고르고 눌렀는데 이백 개가 바뀌는 쪽이 원격이었다
/// (2026-09-01). 이름 바꾸기는 되돌리기가 없으니 좁은 쪽이 옳다.
///
/// `.` 과 `..` 은 언제나 뺀다. 서버 목록에는 이 둘이 그대로 들어 있어서, 규칙이 걸리면
/// 상위 폴더를 이름 바꾸려 든다.
pub(crate) fn batch_targets(
    entries: &[SftpEntry],
    multi: &std::collections::HashSet<String>,
) -> Vec<String> {
    entries
        .iter()
        .map(|e| e.name.as_str())
        .filter(|n| *n != "." && *n != "..")
        .filter(|n| multi.is_empty() || multi.contains(*n))
        .map(str::to_string)
        .collect()
}



/// 원격 항목을 폴더 우선 + 기준(이름/크기/날짜)으로 정렬.
pub(crate) fn sort_sftp(entries: &mut [SftpEntry], sort: crate::browserfs::Sort, desc: bool) {
    // 규칙은 `browserfs::entry_cmp` 한 곳에만 있다 — 로컬 창과 원격 창이 같은 폴더를
    // 다른 순서로 보여 주면 사용자는 어느 쪽을 믿어야 할지 모른다.
    entries.sort_by(|a, b| crate::browsersort::entry_cmp(key_of(a), key_of(b), sort, desc));
}

/// 원격 항목에서 정렬 열쇠를 뽑는다.
fn key_of(e: &SftpEntry) -> crate::browsersort::SortKey<'_> {
    crate::browsersort::SortKey { name: &e.name, is_dir: e.is_dir, size: e.size, mtime: e.mtime }
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
        // 뿌리에 가까운 경로는 규모가 다르다 — 같은 말로 물으면 사람이 멈추지 않는다.
        let peril = crate::sftppath::is_perilous(&crate::sftppath::join_path(&panel.path, &name));
        ui.horizontal(|ui| {
            if peril {
                ui.colored_label(crate::theme_ui::ERR, tr(lang, "sftp.delperil"));
            }
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
    /// HEX(이진) 편집기로 열기 — 임시로 받아 연다.
    EditHex(String),
    /// 앞부분만 받아 미리보기(내려받지 않는다).
    Preview(String),
    Chmod(String, u32),
    /// 소유자·그룹 변경(uid, gid). `None` 인 쪽은 그대로 둔다.
    Chown(String, Option<u32>, Option<u32>),
    /// 권한 재귀 적용(하위 전부).
    ChmodRecursive(String, u32),
    /// 이 파일에 원격 명령을 돌린다(remotecmd의 목록에서 고른 것).
    RunCmd(String, crate::remotecmd::RemoteOp),
    /// 같은 폴더에 사본 만들기(서버 안에서 복사 — 받았다 올리지 않는다). (이름, 크기).
    CopyHere(String, u64, bool),
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
    /// 머리글 오른쪽 클릭으로 켜고 끈 선택 열 이름(`colset::REMOTE`).
    ToggleCol(&'static str),
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
    // 현재 정렬 기준·방향 — 표 헤더에 활성 표시(▴/▾)를 그리는 데 쓴다.
    sort: (crate::browserfs::Sort, bool),
    cols: &[String],
    ids: (&crate::passwdmap::IdMap, &crate::passwdmap::IdMap),
    ren: &mut crate::renameui::RenameUi,
) -> Option<EClick> {
    let now = std::time::UNIX_EPOCH.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    // 필터는 **한 번만** 준비한다. 항목마다 만들면 1만 개짜리 폴더에서 프레임마다
    // 1만 번 같은 문자열을 소문자로 바꾸게 된다(배치 Z F3).
    let nf = crate::browserfilter::NameFilter::new(filter);
    let visible: Vec<&SftpEntry> = entries
        .iter()
        .filter(|e| (show_hidden || !e.name.starts_with('.')) && nf.matches(&e.name))
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
        crate::sftptable::table(ui, &visible, cur_path, lang, compare, selected, multi, scroll_to, sort, cols, ids, ren)
    } else {
        // 스크롤은 **보기 모드가 직접** 만든다. 격자는 보이는 줄만 그리려고 `show_rows` 를
        // 써야 하는데, 여기서 한 겹 더 감싸면 스크롤이 둘이 되어 안쪽이 늘 다 그린다.
        crate::sftpview::render(ui, &visible, cur_path, lang, mode, now, compare, selected, multi)
    }
}

#[cfg(test)]
mod tests {
    use super::batch_targets;
    use nabi_proto::SftpEntry;
    use std::collections::HashSet;

    fn e(name: &str) -> SftpEntry {
        SftpEntry {
            name: name.to_string(),
            is_dir: false,
            is_link: false,
            size: 0,
            mode: 0,
            mtime: 0,
            uid: None,
            gid: None,
        }
    }

    fn sel(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    /// **고른 것이 있으면 그것만.** 로컬 창이 그렇게 하는데 원격만 폴더 전체를 바꾸고 있었다.
    #[test]
    fn a_selection_narrows_the_target() {
        let list = [e("a.txt"), e("b.txt"), e("c.txt")];
        let got = batch_targets(&list, &sel(&["a.txt", "c.txt"]));
        assert_eq!(got, vec!["a.txt", "c.txt"]);
    }

    /// 아무것도 안 골랐으면 이 폴더 전체가 대상이다(예전 동작 유지).
    #[test]
    fn no_selection_means_the_whole_folder() {
        let list = [e("a.txt"), e("b.txt")];
        assert_eq!(batch_targets(&list, &HashSet::new()), vec!["a.txt", "b.txt"]);
    }

    /// **`.` 과 `..` 은 언제나 뺀다.** 서버 목록에 그대로 들어 있어서, 규칙이 걸리면
    /// 상위 폴더를 이름 바꾸려 든다.
    #[test]
    fn the_dot_entries_are_never_a_target() {
        let list = [e("."), e(".."), e("real")];
        assert_eq!(batch_targets(&list, &HashSet::new()), vec!["real"]);
        // 골라 놨더라도 마찬가지다(전체 선택으로 들어올 수 있다).
        assert_eq!(batch_targets(&list, &sel(&[".", "..", "real"])), vec!["real"]);
    }
}
