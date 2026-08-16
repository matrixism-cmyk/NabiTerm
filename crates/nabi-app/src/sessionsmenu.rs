//! Sessions 메뉴 렌더: 새 SSH 추가 · 폴더 그룹화 목록(연결/편집/삭제) · 내보내기/가져오기.

use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
use nabi_session::{SavedSession, SessionKind};

/// Sessions 메뉴 본문을 그리고 선택된 액션을 돌려준다(saved는 이름순 정렬 가정).
pub(crate) fn sessions_menu(ui: &mut egui::Ui, lang: Lang, saved: &[SavedSession], last_conn: &std::collections::BTreeMap<String, i64>, active: &std::collections::HashSet<String>) -> Option<MenuAction> {
    let mut action = None;
    let now = chrono::Local::now().timestamp();
    let last = |s: &SavedSession| last_conn.get(&s.name).copied();
    // 세션이 현재 연결돼 있는가(user@host:port가 활성 집합에 있으면).
    let live = |s: &SavedSession| matches!(&s.kind, nabi_session::SessionKind::Ssh { host, user, port, .. } if active.contains(&format!("{user}@{host}:{port}")));
    if ui.button(tr(lang, "menu.newssh")).clicked() {
        action = Some(MenuAction::NewSshConnection);
        ui.close();
    }
    ui.separator();
    if saved.is_empty() {
        ui.label(tr(lang, "sessions.empty"));
    }
    // 세션 검색바 — 이름/호스트/사용자/폴더를 통합 검색(SessionTree::filter와 동일한 session_matches 기준).
    let q_id = egui::Id::new("sessions_filter");
    let mut q = ui.data(|d| d.get_temp::<String>(q_id)).unwrap_or_default();
    if !saved.is_empty() {
        ui.add(egui::TextEdit::singleline(&mut q).hint_text(tr(lang, "sessions.search")).desired_width(220.0));
        ui.data_mut(|d| d.insert_temp(q_id, q.clone()));
    }
    let hit = |s: &SavedSession| nabi_session::session_matches(s, &q);
    // "그룹 이동" 공용 컨텍스트 메뉴용 전체 폴더 목록.
    let all_folders: Vec<String> = { let mut f: Vec<String> = saved.iter().filter_map(|s| s.folder.clone()).collect(); f.sort(); f.dedup(); f };
    // 폴더 없는 세션부터(검색 일치만).
    for s in saved.iter().filter(|s| s.folder.is_none() && hit(s)) {
        if let Some(a) = session_row(ui, lang, s, last(s), now, live(s), &all_folders) {
            action = Some(a);
        }
    }
    // 폴더별 서브메뉴(일치 항목이 있는 폴더만).
    let mut folders: Vec<&str> = saved.iter().filter(|s| hit(s)).filter_map(|s| s.folder.as_deref()).collect();
    folders.sort_unstable();
    folders.dedup();
    for f in folders {
        let fhdr = egui::RichText::new(format!("\u{1f4c1} {f}")).color(crate::theme_ui::FOLDER);
        ui.menu_button(fhdr, |ui| {
            // 폴더 전체 연결(레이아웃처럼 한 번에 모두 열기, E7).
            if ui.button(tr(lang, "sessions.connectall")).clicked() { action = Some(MenuAction::ConnectFolder(f.to_string())); ui.close(); }
            ui.separator();
            for s in saved.iter().filter(|s| s.folder.as_deref() == Some(f) && hit(s)) {
                if let Some(a) = session_row(ui, lang, s, last(s), now, live(s), &all_folders) {
                    action = Some(a);
                }
            }
            // 그룹 관리(백로그): 이름 바꾸기(일괄)·해산(세션은 유지).
            ui.separator();
            ui.menu_button(tr(lang, "sessions.renamegroup"), |ui| {
                if let Some(new) = crate::sessionctx::inline_name_input(ui, egui::Id::new(("rengrp", f)), "sessions.newgroup", lang) {
                    action = Some(MenuAction::RenameGroup(f.to_string(), new));
                    ui.close();
                }
            });
            if ui.button(tr(lang, "sessions.ungroupall")).clicked() {
                action = Some(MenuAction::DisbandGroup(f.to_string()));
                ui.close();
            }
        });
    }
    ui.separator();
    if let Some(a) = manage_menu(ui, lang) {
        action = Some(a);
    }
    action
}

/// 가져오기·내보내기·정리 관리 동작 — "세션 관리" 메뉴와 사이드바가 공유(DRY·완전 통합).
/// 주제별 서브메뉴로 묶어 평면 비대화를 막는다.
pub(crate) fn manage_menu(ui: &mut egui::Ui, lang: Lang) -> Option<MenuAction> {
    let mut action = None;
    let mut group = |ui: &mut egui::Ui, label: &str, items: Vec<(&str, MenuAction)>| {
        ui.menu_button(tr(lang, label), |ui| {
            for (k, a) in items {
                if ui.button(tr(lang, k)).clicked() {
                    action = Some(a);
                    ui.close();
                }
            }
        });
    };
    group(ui, "menu.import", vec![
        ("menu.importsessions", MenuAction::ImportSessions),
        ("menu.importsshconfig", MenuAction::ImportSshConfig),
        ("menu.importfilezilla", MenuAction::ImportFileZilla),
        ("menu.importmobaxterm", MenuAction::ImportMobaXterm),
        ("menu.importputty", MenuAction::ImportPuTTY),
        ("menu.importxshell", MenuAction::ImportXshell),
    ]);
    group(ui, "menu.export", vec![
        ("menu.exportsessions", MenuAction::ExportSessions),
        ("menu.exportsshconfig", MenuAction::ExportSshConfig),
        ("menu.exportfilezilla", MenuAction::ExportFileZilla),
        ("menu.exportmobaxterm", MenuAction::ExportMobaXterm),
        ("menu.exportputty", MenuAction::ExportPuTTY),
    ]);
    group(ui, "menu.organize", vec![
        ("menu.dedupsessions", MenuAction::DedupSessions),
        ("menu.sortsessions", MenuAction::SortSessions),
        ("menu.sortbyhost", MenuAction::SortSessionsByHost),
    ]);
    // 로컬 포트 포워딩(-L) — 상단 메뉴·사이드바 공용(이전엔 상단 메뉴에만 있었음).
    ui.separator();
    if ui.button(tr(lang, "menu.localforward")).clicked() {
        action = Some(MenuAction::OpenForward);
        ui.close();
    }
    // 볼트(자격증명 금고) — 최상위 메뉴에서 세션 관리 영역으로 흡수(T3-1).
    if ui.button(tr(lang, "menu.vault")).on_hover_text(tr(lang, "vault.about")).clicked() {
        action = Some(MenuAction::OpenVault);
        ui.close();
    }
    action
}

/// 세션 한 줄: 이름(연결)은 왼쪽, 동작 아이콘(SFTP·복제·편집·삭제)은 우측 정렬.
/// 커서가 행 위에 있으면 행 전체에 옅은 배경을 깔아 어느 줄을 다루는지 식별을 돕는다.
fn session_row(ui: &mut egui::Ui, lang: Lang, s: &SavedSession, last: Option<i64>, now: i64, live: bool, folders: &[String]) -> Option<MenuAction> {
    let mut action = None;
    let r = ui.horizontal(|ui| {
        ui.set_min_width(300.0); // 행 폭 확보 — 아이콘을 오른쪽 끝에 정렬.
        // 연결 중이면 녹색 점 선두 표시(컬러 이모지라 테마색 무관, D9).
        let dot = if live { "\u{1f7e2} " } else { "" };
        let nm = if s.is_ftp { format!("{dot}{} (FTP)", s.name) } else { format!("{dot}{}", s.name) };
        // 이름 영역을 넓게(아이콘 공간만 남기고) — 행 대부분에서 연결된다. 이름은 좌측정렬로 직접 그린다
        // (add_sized는 centered_and_justified라 텍스트가 가운데로 몰림 — 그 회피).
        let bw = (ui.available_width() - 118.0).max(80.0);
        let (rect, nb) =
            ui.allocate_exact_size(egui::vec2(bw, ui.spacing().interact_size.y), egui::Sense::click());
        let vis = ui.style().interact(&nb);
        if nb.hovered() {
            ui.painter().rect_filled(rect, vis.corner_radius, vis.bg_fill);
        }
        ui.painter().text(
            egui::pos2(rect.left() + 6.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &nm,
            egui::TextStyle::Button.resolve(ui.style()),
            vis.text_color(),
        );
        // 마지막 접속 상대시간을 호버에 덧붙인다(D4).
        let hint = match last {
            Some(t) if t > 0 => format!("{}\n\u{1f553} {}", crate::connectsave::conn_hint(s), crate::humanfmt::human_age(t as u64, now as u64)),
            _ => crate::connectsave::conn_hint(s),
        };
        let nbr = nb.on_hover_text(hint);
        if nbr.clicked() {
            action = Some(MenuAction::ConnectSaved(s.clone()));
            ui.close();
        }
        // 우클릭 시 사이드바와 동일한 공용 컨텍스트 메뉴(기능 통일).
        nbr.context_menu(|ui| {
            if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
        });
        // 남은 폭을 오른쪽 정렬로 — 아이콘이 행 우측 끝에 모인다(오른쪽→왼쪽 추가 순서).
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let icon = |ui: &mut egui::Ui, g: &str, key: &str| {
                ui.small_button(g).on_hover_text(tr(lang, key)).clicked()
            };
            if icon(ui, "\u{2715}", "sessions.delete") {
                action = Some(MenuAction::DeleteSession(s.name.clone()));
                ui.close();
            }
            if icon(ui, "\u{270e}", "sessions.edit") {
                action = Some(MenuAction::EditSession(s.clone()));
                ui.close();
            }
            // SFTP 열기: 일반 SSH 세션만(FTP 세션은 연결이 곧 FTP 브라우저).
            if matches!(s.kind, SessionKind::Ssh { .. })
                && !s.is_ftp
                && icon(ui, "\u{1f5a7}", "sessions.opensftp")
            {
                action = Some(MenuAction::OpenSftp(s.clone()));
                ui.close();
            }
            // 더보기 "⋯" = 사이드바와 동일한 공용 전체 메뉴(고정·메모·복제·복사·연결테스트·그룹이동 등, 기능 통일).
            ui.menu_button("\u{22ef}", |ui| {
                if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
            });
        });
    });
    if ui.rect_contains_pointer(r.response.rect) {
        let hov = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(r.response.rect, 2.0, hov.linear_multiply(0.5));
    }
    action
}
