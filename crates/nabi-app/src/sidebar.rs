//! 왼쪽 세션 사이드바(MobaXterm식) — 저장 세션 트리: 폴더 그룹·필터·더블클릭 연결.

use crate::app::NabiApp;
use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
use nabi_session::{SavedSession, SessionKind};

impl NabiApp {
    /// 세션 사이드바를 그린다(설정 `show_sessions_panel`이 켜진 경우).
    pub(crate) fn show_sessions_sidebar(&mut self, ctx: &egui::Context) {
        if !self.config.appearance.show_sessions_panel {
            return;
        }
        let lang = self.lang;
        let mut saved = self.sessions.sessions.clone();
        saved.sort_by_key(|s| s.name.to_lowercase());
        let mut action: Option<MenuAction> = None;
        // 그룹 헤더 우클릭 결과(클로저에서 수집 → 닫힌 뒤 적용).
        let (mut start_rename, mut ungroup_folder, mut rename_apply): (Option<String>, Option<String>, Option<(String, String)>) = (None, None, None);
        // 그룹 접기 상태(영속) — 현재 접힌 그룹 + 이번 프레임 토글 요청.
        let collapsed = self.config.appearance.collapsed_groups.clone();
        let pinned = self.config.appearance.pinned_sessions.clone();
        let notes = self.config.appearance.session_notes.clone();
        // 연결중 표시(🟢)·마지막 접속 시간 — "세션 관리" 메뉴와 동일 정보(완전 통합).
        let active: std::collections::HashSet<String> = self.pane_origins.values().filter_map(|k| match k {
            SessionKind::Ssh { host, user, port, .. } => Some(format!("{user}@{host}:{port}")), _ => None }).collect();
        let last_conn = self.config.terminal.last_connected.clone();
        let now = chrono::Local::now().timestamp();
        let mut toggle_group: Option<String> = None;
        egui::SidePanel::left("sessions_sidebar")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr(lang, "status.sessions"))
                            .strong()
                            .color(crate::theme_ui::ACCENT),
                    );
                    if ui
                        .small_button("\u{2795}")
                        .on_hover_text(tr(lang, "menu.newssh"))
                        .clicked()
                    {
                        action = Some(MenuAction::NewSshConnection);
                    }
                    // 관리(⋯): "세션 관리" 메뉴와 동일한 가져오기·내보내기·정리(공유 manage_menu, 완전 통합).
                    ui.menu_button("\u{22ef}", |ui| {
                        if let Some(a) = crate::sessionsmenu::manage_menu(ui, lang) { action = Some(a); }
                    })
                    .response
                    .on_hover_text(tr(lang, "menu.sessions"));
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.sidebar_filter)
                        .hint_text(tr(lang, "browser.filter"))
                        .desired_width(f32::INFINITY),
                );
                ui.separator();
                // 그룹 이름 바꾸기 인라인 편집(헤더 우클릭 ▸ 이름 바꾸기로 진입).
                if let Some(old) = self.sidebar_rename_group.clone() {
                    ui.horizontal(|ui| {
                        ui.label(format!("\u{1f4c1} {old} \u{2192}"));
                        ui.add(egui::TextEdit::singleline(&mut self.sidebar_rename_to).desired_width(90.0));
                        if ui.small_button("\u{2714}").clicked() { rename_apply = Some((old.clone(), self.sidebar_rename_to.clone())); }
                        if ui.small_button("\u{2715}").clicked() { self.sidebar_rename_group = None; }
                    });
                }
                let filt = self.sidebar_filter.to_lowercase();
                let vis: Vec<&SavedSession> = saved.iter().filter(|s| nabi_session::session_matches(s, &filt)).collect();
                let cur_sel = self.sidebar_selected.clone();
                let mut new_sel: Option<String> = None;
                // DnD: 세션 행을 드래그해 그룹 헤더/루트/새 그룹에 드롭하면 folder 변경.
                let mut move_to: Option<(String, Option<String>)> = None;
                let new_group = self.sidebar_new_group.trim().to_string();
                // 우클릭 "그룹 이동" 서브메뉴용 기존 그룹 목록.
                let all_folders: Vec<String> = { let mut f: Vec<String> = saved.iter().filter_map(|s| s.folder.clone()).collect(); f.sort(); f.dedup(); f };
                // 드래그 가능한 세션 한 줄(side_row를 드래그 소스로 감싼다).
                let drag_row = |ui: &mut egui::Ui, s: &SavedSession, sel: Option<&str>, ns: &mut Option<String>| -> Option<MenuAction> {
                    let live = matches!(&s.kind, SessionKind::Ssh { host, user, port, .. } if active.contains(&format!("{user}@{host}:{port}")));
                    let last = last_conn.get(&s.name).copied();
                    // 드래그 소스는 side_row 내부에서 이름 라벨에만 적용 — 우측 아이콘 클릭이 드래그에 가로채이지 않게.
                    side_row(ui, lang, s, sel, ns, &all_folders, &notes, live, last, now)
                };
                egui::ScrollArea::vertical().id_salt("sidebar_sessions").show(ui, |ui| {
                    if vis.is_empty() {
                        // 필터로 0건인지(있지만 안 보임) 저장 자체가 없는지 구분.
                        ui.weak(tr(lang, if filt.trim().is_empty() { "sessions.empty" } else { "sessions.nomatch" }));
                        return;
                    }
                    // 📌 고정(즐겨찾기) — folder와 무관하게 최상단에 모아 보여준다(MobaXterm식).
                    if vis.iter().any(|s| pinned.contains(&s.name)) {
                        ui.label(egui::RichText::new(format!("\u{1f4cc} {}", tr(lang, "sessions.pinned"))).color(crate::theme_ui::FOLDER));
                        for s in vis.iter().filter(|s| pinned.contains(&s.name)) { if let Some(a) = drag_row(ui, s, cur_sel.as_deref(), &mut new_sel) { action = Some(a); } }
                        ui.separator();
                    }
                    // 그룹 없음(루트) — 드롭 시 그룹 해제. 고정 항목은 위 📌 그룹에만.
                    let (_, drop) = ui.dnd_drop_zone::<String, _>(egui::Frame::none(), |ui| {
                        for s in vis.iter().filter(|s| s.folder.is_none() && !pinned.contains(&s.name)) {
                            if let Some(a) = drag_row(ui, s, cur_sel.as_deref(), &mut new_sel) { action = Some(a); }
                        }
                    });
                    if let Some(n) = drop { move_to = Some(((*n).clone(), None)); }
                    let mut folders: Vec<&str> = vis.iter().filter_map(|s| s.folder.as_deref()).collect();
                    folders.sort_unstable();
                    folders.dedup();
                    for &f in &folders {
                        let fhdr = egui::RichText::new(format!("\u{1f4c1} {f}")).color(crate::theme_ui::FOLDER);
                        let (_, drop) = ui.dnd_drop_zone::<String, _>(egui::Frame::none(), |ui| {
                            // 접기 상태는 config로 영속 — open(Some)으로 강제, 헤더 클릭 시 토글 수집.
                            // 필터 중에는 접힌 폴더도 강제로 펼쳐 일치 항목이 숨지 않게 한다(트리 검색 UX).
                            let want_open = !filt.trim().is_empty() || !collapsed.iter().any(|c| c == f);
                            let ch = egui::CollapsingHeader::new(fhdr).id_salt(("sfolder", f)).open(Some(want_open)).show(ui, |ui| {
                                for s in vis.iter().filter(|s| s.folder.as_deref() == Some(f) && !pinned.contains(&s.name)) {
                                    if let Some(a) = drag_row(ui, s, cur_sel.as_deref(), &mut new_sel) { action = Some(a); }
                                }
                            });
                            if ch.header_response.clicked() { toggle_group = Some(f.to_string()); }
                            ch.header_response.context_menu(|ui| {
                                if ui.button(tr(lang, "sessions.connectall")).clicked() { action = Some(MenuAction::ConnectFolder(f.to_string())); ui.close_menu(); }
                                if ui.button(tr(lang, "sessions.renamegroup")).clicked() { start_rename = Some(f.to_string()); ui.close_menu(); }
                                if ui.button(tr(lang, "sessions.ungroupall")).clicked() { ungroup_folder = Some(f.to_string()); ui.close_menu(); }
                            });
                        });
                        if let Some(n) = drop { move_to = Some(((*n).clone(), Some(f.to_string()))); }
                    }
                    // 새 그룹: 이름 입력 시 드롭 대상 헤더로 표시(드롭하면 그 이름의 그룹 생성·이동).
                    if !new_group.is_empty() && !folders.contains(&new_group.as_str()) {
                        let hdr = egui::RichText::new(format!("\u{2795} {new_group}")).color(crate::theme_ui::FOLDER);
                        let (ir, drop) = ui.dnd_drop_zone::<String, _>(egui::Frame::group(ui.style()), |ui| { ui.label(hdr); });
                        ir.response.on_hover_text(tr(lang, "sessions.newgroupdrop"));
                        if let Some(n) = drop { move_to = Some(((*n).clone(), Some(new_group.clone()))); }
                    }
                });
                // 새 그룹 이름 입력칸(여기에 입력 후 세션을 위 헤더로 드래그).
                ui.horizontal(|ui| { ui.label("\u{2795}"); ui.add(egui::TextEdit::singleline(&mut self.sidebar_new_group).hint_text(tr(lang, "sessions.newgroup")).desired_width(f32::INFINITY)); });
                if let Some(sel) = new_sel {
                    self.sidebar_selected = Some(sel);
                }
                if let Some((name, folder)) = move_to {
                    let was_new = !new_group.is_empty() && folder.as_deref() == Some(new_group.as_str());
                    self.set_session_folder(&name, folder);
                    if was_new { self.sidebar_new_group.clear(); }
                }
            });
        // 그룹 헤더 우클릭 결과 적용(패널 닫힌 뒤 — 세션 목록 가변 차용 분리).
        if let Some(old) = start_rename { self.sidebar_rename_to = old.clone(); self.sidebar_rename_group = Some(old); }
        if let Some(f) = ungroup_folder { self.rename_folder(&f, ""); }
        if let Some((old, new)) = rename_apply { self.rename_folder(&old, &new); self.sidebar_rename_group = None; }
        if let Some(g) = toggle_group { let v = &mut self.config.appearance.collapsed_groups; if let Some(i) = v.iter().position(|x| x == &g) { v.remove(i); } else { v.push(g); } let _ = nabi_config::save(&self.config_path, &self.config); }
        if let Some(a) = action { self.apply(ctx, a); }
    }

    /// 그룹 이름을 바꾼다(그 folder의 모든 세션 folder 교체; 새 이름이 비면 그룹 해제). 저장.
    fn rename_folder(&mut self, old: &str, new: &str) {
        let folder = (!new.trim().is_empty()).then(|| new.trim().to_string());
        let mut changed = false;
        for s in self.sessions.sessions.iter_mut().filter(|s| s.folder.as_deref() == Some(old)) {
            s.folder = folder.clone();
            changed = true;
        }
        if changed { self.save_sessions(); }
    }

    /// 세션의 그룹(folder)을 바꾸고 저장한다(사이드바 DnD·우클릭 그룹 이동).
    pub(crate) fn set_session_folder(&mut self, name: &str, folder: Option<String>) {
        if let Some(s) = self.sessions.sessions.iter_mut().find(|s| s.name == name) {
            s.folder = folder;
            self.save_sessions();
        }
    }
}

/// 세션 종류 아이콘(로컬/SSH/FTP).
fn kind_icon(s: &SavedSession) -> &'static str {
    if s.is_ftp {
        "\u{1f310}" // 🌐 FTP
    } else {
        match s.kind {
            SessionKind::Local { .. } => "\u{1f4bb}", // 💻 로컬 셸
            SessionKind::Ssh { .. } => "\u{1f5a5}",   // 🖥 SSH
        }
    }
}

/// 사이드바 세션 한 줄: 클릭=연결(SSH 열기), 드래그=그룹 이동, 우클릭=메뉴, 우측 아이콘(✎편집·✕삭제·🖧SFTP·⋯더보기).
#[allow(clippy::too_many_arguments)]
fn side_row(
    ui: &mut egui::Ui,
    lang: Lang,
    s: &SavedSession,
    cur_sel: Option<&str>,
    new_sel: &mut Option<String>,
    folders: &[String],
    notes: &std::collections::BTreeMap<String, String>,
    live: bool,
    last: Option<i64>,
    now: i64,
) -> Option<MenuAction> {
    let mut action = None;
    let is_ssh = matches!(s.kind, SessionKind::Ssh { .. }) && !s.is_ftp;
    let selected = cur_sel == Some(s.name.as_str());
    let resp = ui.horizontal(|ui| {
        // 이름 영역을 전폭 할당(우측 아이콘 공간만 남김) — 클릭=연결·드래그=이동·우클릭=메뉴·호버=라인 강조.
        let icon_w = if is_ssh { 90.0 } else { 68.0 };
        let bw = (ui.available_width() - icon_w).max(48.0);
        let (rect, r) = ui.allocate_exact_size(egui::vec2(bw, ui.spacing().interact_size.y), egui::Sense::click_and_drag());
        let vis = ui.style().interact_selectable(&r, selected);
        // 선택/호버 시 라인 전체 배경 강조(어느 줄을 다루는지 식별).
        if selected {
            ui.painter().rect_filled(rect, vis.rounding, vis.bg_fill);
        } else if r.hovered() {
            ui.painter().rect_filled(rect, vis.rounding, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let font = egui::TextStyle::Button.resolve(ui.style());
        // 연결 중이면 종류 아이콘을 강조색(초록)으로 — 🟢 점 대신 선두 아이콘 색으로 표시.
        let kcolor = if live { crate::theme_ui::OK } else { crate::theme_ui::session_color(s.is_ftp, is_ssh) };
        ui.painter().text(egui::pos2(rect.left() + 5.0, rect.center().y), egui::Align2::LEFT_CENTER, kind_icon(s), font.clone(), kcolor);
        // 이름(+메모 📝) — 폭 넘치면 … 말줄임.
        let note = if notes.get(&s.name).is_some_and(|n| !n.is_empty()) { " \u{1f4dd}" } else { "" };
        let mut job = egui::text::LayoutJob::simple_singleline(format!("{}{note}", s.name), font, vis.text_color());
        job.wrap = egui::text::TextWrapping { max_width: (rect.width() - 26.0).max(16.0), max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
        let galley = ui.fonts(|f| f.layout_job(job));
        ui.painter().galley(egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, vis.text_color());
        // 드래그(길게 눌러 이동) 페이로드 + 고스트(커서 옆에 이름).
        if r.drag_started() { r.dnd_set_drag_payload(s.name.clone()); }
        if r.dragged() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                egui::Area::new(egui::Id::new(("sdghost", &s.name))).order(egui::Order::Tooltip).fixed_pos(p + egui::vec2(12.0, 4.0))
                    .show(ui.ctx(), |ui| { egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(format!("\u{1f5a5} {}", s.name))); });
            }
        }
        // 인라인 동작 아이콘(우측) — 간격 좁힘: ⋯더보기 · 🖧SFTP(SSH) · ✎편집 · ✕삭제.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let icon = |ui: &mut egui::Ui, g: &str, key: &str| ui.small_button(g).on_hover_text(tr(lang, key)).clicked();
            if icon(ui, "\u{2715}", "sessions.delete") { action = Some(MenuAction::DeleteSession(s.name.clone())); }
            if icon(ui, "\u{270e}", "sessions.edit") { action = Some(MenuAction::EditSession(s.clone())); }
            if is_ssh && icon(ui, "\u{1f5a7}", "sessions.opensftp") { action = Some(MenuAction::OpenSftp(s.clone())); }
            ui.menu_button("\u{22ef}", |ui| { if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); } });
        });
        r
    });
    // 호버: 연결 정보 + (있으면) 마지막 접속 상대시간 + 메모.
    let mut hint = crate::connectsave::conn_hint(s);
    if let Some(t) = last.filter(|&t| t > 0) { hint.push_str(&format!("\n\u{1f553} {}", crate::humanfmt::human_age(t as u64, now as u64))); }
    if let Some(n) = notes.get(&s.name).filter(|n| !n.is_empty()) { hint.push_str(&format!("\n\u{1f4dd} {n}")); }
    let resp = resp.inner.on_hover_text(hint);
    // 클릭=연결(SSH 열기). 드래그(이동)는 페이로드로 처리돼 단순 클릭과 구분된다.
    if resp.clicked() {
        *new_sel = Some(s.name.clone());
        action = Some(MenuAction::ConnectSaved(s.clone()));
    }
    // 우클릭=더보기(라인 어디서나). 라인 전폭 영역이라 이름 어디서 눌러도 동일 메뉴.
    resp.context_menu(|ui| {
        if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
    });
    action
}

