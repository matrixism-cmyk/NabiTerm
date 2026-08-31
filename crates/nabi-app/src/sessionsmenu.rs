//! Sessions 메뉴 렌더: 새 SSH 추가 · 폴더 그룹화 목록(연결/편집/삭제) · 내보내기/가져오기.

use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
use nabi_session::SavedSession;

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
    // 제품별 항목(PuTTY·MobaXterm·FileZilla·WinSCP·Xshell·ssh config)은 **가져오기 화면
    // 안에만** 둔다. 그 화면은 이 PC를 훑어 **찾은 것을 먼저** 보여 주고, 못 찾은 것도
    // "직접 고르기"로 남긴다 — 메뉴에 같은 여섯 줄을 또 두면 열 줄짜리 메뉴가 되는데,
    // 그러면 무엇을 눌러야 할지 고르는 일이 오히려 어려워진다.
    //
    // 화면 자체가 그 문제를 풀려고 만든 것이었다(감사 2026-08-25). 그런데 만들면서 옛
    // 항목을 걷어내지 않아 **둘이 나란히 남았다** — 정리는 그때 함께 했어야 한다.
    group(ui, "menu.import", vec![
        ("menu.importscreen", MenuAction::OpenImportScreen),
        ("menu.importsessions", MenuAction::ImportSessions),
        ("menu.restoreall", MenuAction::RestoreAll),
    ]);
    group(ui, "menu.export", vec![
        ("menu.exportsessions", MenuAction::ExportSessions),
        ("menu.backupall", MenuAction::BackupAll),
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
    if ui.button(tr(lang, "keygen.title")).clicked() {
        action = Some(MenuAction::OpenKeygen);
        ui.close();
    }
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
        // 연결 중이면 점을 앞에 붙인다.
        //
        // 예전에는 컬러 이모지(U+1F7E2 🟢)를 썼는데 **그 글자가 어느 폴백 글꼴에도 없어**
        // 네모 상자로 나왔다(`xtask glyphs` 가 찾았다, 2026-08-31). 어디에나 있는 ● 를
        // 쓰고 색은 우리가 칠한다 — 글꼴에 기대지 않는 쪽이 늘 안전하다.
        let nm = if s.is_ftp { format!("{} (FTP)", s.name) } else { s.name.clone() };
        // 이름 영역을 넓게(아이콘 공간만 남기고) — 행 대부분에서 연결된다. 이름은 좌측정렬로 직접 그린다
        // (add_sized는 centered_and_justified라 텍스트가 가운데로 몰림 — 그 회피).
        let bw = (ui.available_width() - 44.0).max(80.0); // 아이콘이 하나뿐이라 이름을 넓게.
        let (rect, nb) =
            ui.allocate_exact_size(egui::vec2(bw, ui.spacing().interact_size.y), egui::Sense::click());
        let vis = ui.style().interact(&nb);
        if nb.hovered() {
            ui.painter().rect_filled(rect, vis.corner_radius, vis.bg_fill);
        }
        // 점만 초록으로, 이름은 보통 색으로 — 한 번에 그리려고 조각을 이어 붙인다.
        // 예전에는 컬러 이모지가 색을 갖고 있어 이럴 필요가 없었다.
        let font = egui::TextStyle::Button.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        if live {
            job.append(
                "\u{25cf} ",
                0.0,
                egui::TextFormat { font_id: font.clone(), color: crate::theme_ui::OK, ..Default::default() },
            );
        }
        job.append(
            &nm,
            0.0,
            egui::TextFormat { font_id: font, color: vis.text_color(), ..Default::default() },
        );
        let galley = ui.painter().layout_job(job);
        let y = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(egui::pos2(rect.left() + 6.0, y), galley, vis.text_color());
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
        // 행마다 아이콘 넷(삭제·편집·SFTP·더보기)을 늘어놓으니 목록이 복잡했다 —
        // 세션이 여남은 개만 돼도 이름보다 아이콘이 먼저 눈에 들어온다(사용자 지적 2026-08-25).
        // 사이드바처럼 **이름 위주**로 두고, 동작은 "..."과 우클릭에 모은다(둘 다 같은 메뉴다).
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
