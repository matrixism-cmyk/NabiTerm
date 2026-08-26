//! SFTP 목록 구역 — 인라인 이름변경 편집기 + 드롭존 + 항목 클릭 매핑(sftptab에서 분리).

use crate::sftpentries::EClick;
use crate::sftppanel::SftpPanel;
use crate::sftptab::SftpAct;
use nabi_i18n::Lang;

/// 항목 목록을 드롭존 안에 그리고(자세히 보기는 인라인 이름변경) 결과를 SftpAct에 모은다.
pub(crate) fn list_zone(
    ui: &mut egui::Ui,
    sftp: &mut SftpPanel,
    lang: Lang,
    details: bool,
    sort: (crate::browserfs::Sort, bool),
    a: &mut SftpAct,
) {
    crate::browsercols::apply_list_font(ui, sftp.font_size); // 이 SFTP 목록만 글꼴 적용.
    // 빈 공간 우클릭 메뉴: 행보다 먼저 배경 클릭영역을 등록(나중 등록되는 행이 위에서 좌/우클릭을 가짐).
    let bg = ui.interact(ui.available_rect_before_wrap(), ui.id().with("sftp_empty_bg"), egui::Sense::click());
    bg.context_menu(|ui| {
        let t = |k| nabi_i18n::tr(lang, k);
        if ui.button(format!("\u{1f4c1}+ {}", t("sftp.newfolder"))).clicked() { a.new_folder = true; ui.close(); }
        if ui.button(format!("\u{1f4c4}+ {}", t("sftp.newfile"))).clicked() { a.new_file = true; ui.close(); }
        if ui.button(format!("\u{1f4cb}\u{2193} {}", t("browser.paste"))).clicked() { a.paste = true; ui.close(); } // OS 파일 붙여넣기=업로드.
        if ui.button(format!("\u{2b07} {}", t("sftp.downloaddir"))).clicked() { a.dl_cur = true; ui.close(); }
        if ui.button(format!("\u{21bb} {}", t("sftp.refresh"))).clicked() { a.go = Some(sftp.path.clone()); ui.close(); }
        ui.separator();
        if ui.button(format!("\u{1f50d} {}", t("sftp.search"))).clicked() { a.search = true; ui.close(); }
        if ui.button(format!("\u{1f441} {}", t("sftp.hidden"))).clicked() { sftp.show_hidden = !sftp.show_hidden; ui.close(); }
        if ui.button(format!("\u{1f4cb} {}", t("browser.copycurpath"))).clicked() { ui.ctx().copy_text(sftp.path.clone()); ui.close(); }
        ui.menu_button(t("editor.toolsmenu"), |ui| {
            // 선택/복사는 모두 "보이는 항목"만 대상으로 한다(필터로 가려진 파일 보호).
            if ui.button(format!("\u{2611} {}", t("menu.selectall"))).clicked() { sftp.multi = crate::sftptable::visible_names(sftp).into_iter().collect(); ui.close(); }
            if ui.button(format!("\u{21c4} {}", t("menu.invertsel"))).clicked() { let cur = sftp.multi.clone(); sftp.multi = crate::sftptable::visible_names(sftp).into_iter().filter(|n| !cur.contains(n)).collect(); ui.close(); }
            // 둘을 골랐을 때만 뜼다 — 늘 보이면 눌러 놓고 왜 안 되는지 묻게 된다.
            if sftp.multi.len() == 2 && ui.button(format!("\u{21c4} {}", t("diff.compare"))).clicked() {
                a.compare = true;
                ui.close();
            }
            if ui.button(format!("\u{1f4cb} {}", t("menu.copypaths"))).clicked() {
                // 선택이 없으면 보이는 항목 전체(로컬 브라우저와 같은 규칙).
                let names: Vec<String> = if sftp.multi.is_empty() {
                    crate::sftptable::visible_names(sftp)
                } else {
                    sftp.multi.iter().cloned().collect()
                };
                let paths = names.iter().map(|n| crate::sftppath::join_path(&sftp.path, n));
                ui.ctx().copy_text(paths.collect::<Vec<_>>().join("\n"));
                ui.close();
            }
        });
    });
    // 인라인 이름변경 편집기(자세히 보기): sftp.input 버퍼를 셀 위치에서 직접 편집.
    let rn_target = if details { sftp.rename_from.clone() } else { None };
    let rn_buf = rn_target.as_ref().map(|_| &mut sftp.input);
    let mut ren = crate::renameui::RenameUi {
        target: rn_target,
        buf: rn_buf,
        focus: &mut sftp.rename_focus,
        commit: false,
        cancel: false,
    };
    // 목록 영역을 드롭 존으로 — 로컬 브라우저에서 끌어온 경로(String)를 받으면 업로드.
    let (dropped, payload) = ui.dnd_drop_zone::<String, _>(egui::Frame::NONE, |ui| {
        crate::sftpentries::show_entries(
            ui,
            &sftp.entries,
            &sftp.path,
            lang,
            &sftp.filter,
            sftp.compare.as_ref(),
            sftp.show_hidden,
            sftp.view_mode,
            sftp.selected.as_deref(),
            &sftp.multi,
            sftp.scroll,
            sort,
            &mut ren,
        )
    });
    a.do_rename |= ren.commit; // 인라인 편집 확정.
    if ren.cancel {
        sftp.rename_from = None; // 인라인 편집 취소.
        sftp.input.clear();
    }
    sftp.scroll = false; // 스크롤 요청 소비.
    if let Some(p) = payload {
        a.upload_path = Some((*p).clone());
    }
    match dropped.inner {
        Some(EClick::Nav(p)) => a.go = Some(p),
        Some(EClick::Download(n, s)) => a.dl = Some((n, s)),
        Some(EClick::DownloadDir(n)) => a.dldir = Some(n),
        Some(EClick::DirSize(n)) => a.dirsize = Some(n),
        Some(EClick::OpenTermHere(n)) => a.open_term = Some(n),
        Some(EClick::Edit(n)) => a.edit = Some(n),
        Some(EClick::Preview(n)) => a.preview = Some(n),
        Some(EClick::Chmod(n, m)) => a.chmod = Some((n, m)),
        Some(EClick::ChmodRecursive(n, m)) => a.chmod_rec = Some((n, m)),
        Some(EClick::RunCmd(n, op)) => a.run_cmd = Some((n, op)),
        Some(EClick::CopyHere(n, s, d)) => a.copy_here = Some((n, s, d)),
        Some(EClick::Rename(n)) => a.rename = Some(n),
        Some(EClick::Delete(n)) => a.del = Some(n),
        Some(EClick::DropInto(folder, local)) => a.upload_into = Some((folder, local)),
        Some(EClick::SetSort(s)) => a.set_sort = Some(s),
        Some(EClick::Select(n, c, s)) => a.select = Some((n, c, s)),
        Some(EClick::OsDrag(n, s, d)) => a.os_drag = Some((n, s, d)),
        None => {}
    }
    let _ = dropped; // 드롭 응답에는 메뉴를 붙이지 않는다(행 영역을 덮으면 클릭 가로챔). 빈공간 메뉴는 상단 bg에.
}
