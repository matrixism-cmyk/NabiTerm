//! SFTP 항목 보기 모드(윈도우 탐색기식): 자세히/목록/큰·작은 아이콘/타일.
//! 항목 클릭·컨텍스트 메뉴 처리는 공유하고, 레이아웃만 모드별로 다르게 그린다.

use crate::sftpentries::EClick;
use crate::sftpentryfmt::{cmp_color, cmp_status};
use crate::sftppath::join_path;
use nabi_i18n::{tr, Lang};
use nabi_proto::SftpEntry;
use std::collections::HashMap;

/// 원격 항목을 로컬로 드래그할 때 싣는 페이로드(로컬 브라우저가 받아 다운로드).
#[derive(Clone)]
pub(crate) struct RemoteName {
    pub name: String,
    pub is_dir: bool,
}

// ViewMode는 viewmode.rs로 분리(로컬·SFTP 공용). 기존 경로 유지를 위해 재export.
pub(crate) use crate::viewmode::ViewMode;

/// 항목 아이콘(폴더=📁, 파일=확장자별).
pub(crate) fn icon(e: &SftpEntry) -> &'static str {
    if e.is_link {
        "\u{1f517}" // 🔗 심볼릭 링크(대상 종류 무관) — FileZilla식 구분.
    } else if e.is_dir {
        "\u{1f4c1}"
    } else {
        crate::filetype::file_icon(&e.name)
    }
}

/// 한 항목의 클릭/컨텍스트 메뉴를 처리해 EClick을 돌려준다(모든 모드 공유).
pub(crate) fn actions(resp: &egui::Response, e: &SftpEntry, cur: &str, lang: Lang) -> Option<EClick> {
    let mut click = None;
    // 로컬 파일을 이 폴더 행에 드롭하면 그 폴더로 업로드(드롭존).
    if e.is_dir {
        if let Some(local) = resp.dnd_release_payload::<String>() {
            return Some(EClick::DropInto(e.name.clone(), (*local).clone()));
        }
    }
    if resp.clicked() {
        let m = resp.ctx.input(|i| i.modifiers);
        click = Some(EClick::Select(e.name.clone(), m.command, m.shift)); // 선택(Ctrl/Shift).
    }
    // 드래그 시작 → 탐색기로 드래그-아웃(드롭 시 다운로드). 파일=가상파일, 폴더=재귀 다운로드.
    if resp.drag_started() {
        click = Some(EClick::OsDrag(e.name.clone(), e.size, e.is_dir));
    }
    if resp.double_clicked() {
        click = Some(if e.is_dir {
            EClick::Nav(join_path(cur, &e.name))
        } else {
            EClick::Download(e.name.clone(), e.size)
        });
    }
    // 우클릭 시 해당 항목을 먼저 선택(메뉴가 그 파일을 대상으로 동작하도록).
    if resp.secondary_clicked() {
        click = Some(EClick::Select(e.name.clone(), false, false));
    }
    resp.context_menu(|ui| {
        // 심볼릭 링크: 디렉터리로 진입 시도(대부분 dir 링크). 더블클릭 기본은 다운로드라 회귀 없음.
        if e.is_link && ui.button(tr(lang, "sftp.enterlink")).clicked() {
            click = Some(EClick::Nav(join_path(cur, &e.name)));
            ui.close();
        }
        // 파일 다운로드(폴더는 아래 '폴더 다운로드'). 메뉴 최상단에 노출.
        // 서버에서 명령 돌리기 — 고르면 확인창에서 전문을 보여 준 뒤 실행한다.
        if let Some(op) = crate::app::NabiApp::remote_cmd_menu(ui, lang) {
            click = Some(EClick::RunCmd(e.name.clone(), op));
            ui.close();
        }
        // 서버 안에서 사본 만들기 — 폴더면 하위까지 재귀로 옮긴다.
        if ui.button(tr(lang, "sftp.copyhere")).clicked() {
            click = Some(EClick::CopyHere(e.name.clone(), e.size, e.is_dir));
            ui.close();
        }
        if !e.is_dir && ui.button(tr(lang, "sftp.download")).clicked() {
            click = Some(EClick::Download(e.name.clone(), e.size));
            ui.close();
        }
        if ui.button(tr(lang, "browser.copypath")).clicked() {
            ui.ctx().copy_text(join_path(cur, &e.name));
            ui.close();
        }
        if e.is_dir && ui.button(tr(lang, "sftp.downloaddir")).clicked() {
            click = Some(EClick::DownloadDir(e.name.clone()));
            ui.close();
        }
        if e.is_dir && ui.button(tr(lang, "sftp.dirsize")).clicked() {
            click = Some(EClick::DirSize(e.name.clone()));
            ui.close();
        }
        if e.is_dir && ui.button(tr(lang, "sftp.openterm")).clicked() {
            click = Some(EClick::OpenTermHere(e.name.clone()));
            ui.close();
        }
        if !e.is_dir && ui.button(tr(lang, "sftp.preview")).clicked() {
            click = Some(EClick::Preview(e.name.clone()));
            ui.close();
        }
        if !e.is_dir && ui.button(tr(lang, "sftp.edit")).clicked() {
            click = Some(EClick::Edit(e.name.clone()));
            ui.close();
        }
        ui.menu_button(tr(lang, "sftp.perms"), |ui| {
            if let Some(a) = crate::sftpperms::perms_menu(ui, &e.name, e.is_dir, lang) {
                click = Some(a);
            }
        });
        if ui.button(tr(lang, "sftp.rename")).clicked() {
            click = Some(EClick::Rename(e.name.clone()));
            ui.close();
        }
        if ui.button(tr(lang, "sftp.delete")).clicked() {
            click = Some(EClick::Delete(e.name.clone()));
            ui.close();
        }
    });
    click
}

/// 모드에 맞게 항목들을 그린다(필터된 목록). compare가 있으면 비교 색칠.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    ui: &mut egui::Ui,
    entries: &[&SftpEntry],
    cur: &str,
    lang: Lang,
    mode: ViewMode,
    now: u64,
    compare: Option<&HashMap<String, (bool, u64)>>,
    selected: Option<&str>,
    // 다중 선택 집합 — 자세히 보기만 강조하고 아이콘·타일은 빠져 있던 것을 맞춘다.
    multi: &std::collections::HashSet<String>,
) -> Option<EClick> {
    let sel = |n: &str| Some(n) == selected || multi.contains(n);
    match mode {
        ViewMode::Details => details(ui, entries, cur, lang, now, compare, false),
        ViewMode::Content => details(ui, entries, cur, lang, now, compare, true),
        ViewMode::List => grid(ui, entries, cur, lang, 150.0, 18.0, 13.0, false, &sel),
        ViewMode::LargeIcons => grid(ui, entries, cur, lang, 96.0, 78.0, 22.0, true, &sel),
        ViewMode::SmallIcons => grid(ui, entries, cur, lang, 70.0, 54.0, 15.0, true, &sel),
        ViewMode::Tile => grid(ui, entries, cur, lang, 168.0, 22.0, 13.0, false, &sel),
    }
}

/// 자세히/내용: 전체 너비 행. two_line이면 이름/메타 2줄(내용 보기).
#[allow(clippy::too_many_arguments)]
fn details(
    ui: &mut egui::Ui,
    entries: &[&SftpEntry],
    cur: &str,
    lang: Lang,
    now: u64,
    compare: Option<&HashMap<String, (bool, u64)>>,
    two_line: bool,
) -> Option<EClick> {
    let mut click = None;
    for e in entries {
        let perm = if e.mode & 0o777 != 0 {
            format!("  {:03o}", e.mode & 0o777)
        } else {
            String::new()
        };
        let age = crate::browserfs::human_age(e.mtime, now);
        let size = if e.is_dir {
            String::new()
        } else {
            format!("  ({})", crate::browserfs::human(e.size))
        };
        let text = if two_line {
            format!("{} {}\n    {}{perm}  {age}", icon(e), e.name, size.trim())
        } else {
            format!("{} {}{size}{perm}  {age}", icon(e), e.name)
        };
        let color = if e.is_dir {
            crate::filetype::FOLDER_COLOR
        } else {
            crate::filetype::file_color(&e.name)
        };
        let mut rich = egui::RichText::new(text).color(color);
        if let Some(map) = compare {
            if let Some(c) = cmp_color(cmp_status(&e.name, e.size, e.is_dir, map)) {
                rich = rich.color(c);
            }
        }
        let w = ui.available_width();
        let h = if two_line { 34.0 } else { 18.0 };
        // click_and_drag로 직접 센스(드래그 커서로 클릭 막히는 문제 방지).
        let resp = ui.add_sized(
            [w, h],
            egui::Label::new(rich).selectable(false).sense(egui::Sense::click_and_drag()),
        );
        if resp.dragged() {
            resp.dnd_set_drag_payload(RemoteName {
                name: e.name.clone(),
                is_dir: e.is_dir,
            });
        }
        // 권한이 있으면 호버 시 `ls -l`식 rwx + 8진수를 보여준다(가독성).
        let resp = if perm.is_empty() {
            resp
        } else {
            let rwx = crate::sftpentryfmt::mode_to_rwx(e.mode, e.is_dir, e.is_link);
            resp.on_hover_text(format!("{rwx}  ({:03o})", e.mode & 0o777))
        };
        if let Some(a) = actions(&resp, e, cur, lang) {
            click = Some(a);
        }
    }
    click
}

/// 격자/타일/목록: 셀 크기·글자 크기로 모양을 조절(with_size면 크기도 표시).
#[allow(clippy::too_many_arguments)]
fn grid(
    ui: &mut egui::Ui,
    entries: &[&SftpEntry],
    cur: &str,
    lang: Lang,
    cw: f32,
    ch: f32,
    txt: f32,
    with_size: bool,
    // 이 항목이 선택되었는가(단일 선택 + 다중 선택 합치기).
    selected: &dyn Fn(&str) -> bool,
) -> Option<EClick> {
    let mut click = None;
    let big = ch > 40.0; // 아이콘 모드: 아이콘 위 / 이름 아래(2줄).
    ui.horizontal_wrapped(|ui| {
        // 맨 앞 ".." 셀: 더블클릭으로 상위 디렉터리 이동.
        if cur != "/" && cur != "." {
            let up = egui::RichText::new("\u{2b06} ..").size(txt);
            if ui.add_sized([cw, ch], egui::Button::new(up)).double_clicked() {
                click = Some(EClick::Nav(crate::sftppath::parent_dir(cur)));
            }
        }
        for e in entries {
            let sz = if with_size && !e.is_dir {
                format!("\n{}", crate::browserfs::human(e.size))
            } else {
                String::new()
            };
            let sep = if big { '\n' } else { ' ' };
            let color = if e.is_dir {
                crate::filetype::FOLDER_COLOR
            } else {
                crate::filetype::file_color(&e.name)
            };
            let label = egui::RichText::new(format!("{}{sep}{}{sz}", icon(e), e.name))
                .size(txt)
                .color(color);
            // click_and_drag로 직접 센스(드래그 커서로 클릭 막히는 문제 방지). 로컬 격자와 동일.
            let mut btn = egui::Button::new(label).sense(egui::Sense::click_and_drag());
            if selected(&e.name) {
                btn = btn.fill(ui.visuals().selection.bg_fill); // 선택 항목 강조.
            }
            let resp = ui.add_sized([cw, ch], btn);
            if resp.dragged() {
                resp.dnd_set_drag_payload(RemoteName {
                    name: e.name.clone(),
                    is_dir: e.is_dir,
                });
            }
            if let Some(a) = actions(&resp, e, cur, lang) {
                click = Some(a);
            }
        }
    });
    click
}
