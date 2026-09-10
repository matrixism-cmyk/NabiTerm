//! 왼쪽 세션 사이드바(MobaXterm식) — 저장 세션 트리: 폴더 그룹·필터·더블클릭 연결.

use crate::app::NabiApp;
use crate::menu::MenuAction;
use nabi_i18n::tr;
use crate::sidebarrow::{side_row, RowState};
use nabi_session::{SavedSession, SessionKind};

impl NabiApp {
    /// 세션 사이드바를 그린다(설정 `show_sessions_panel`이 켜진 경우).
    pub(crate) fn show_sessions_sidebar(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        if !self.config.appearance.show_sessions_panel {
            return;
        }
        let lang = self.lang;
        let mut saved = self.sessions.sessions.clone();
        saved.sort_by_key(|s| s.name.to_lowercase());
        let mut action: Option<MenuAction> = None;
        // 선택 막대의 '선택 연결' 클릭(패널 클로저 밖에서 실행 — 세션 목록 가변 차용 분리).
        let mut connect_marked = false;
        // ⋯ 메뉴가 열린 행(직전 프레임) / 이번 프레임에 열려 있는 행.
        let menu_row = self.sidebar_menu_row.clone();
        let mut menu_now: Option<String> = None;
        // 그룹 헤더 우클릭 결과(클로저에서 수집 → 닫힌 뒤 적용).
        let (mut start_rename, mut ungroup_folder, mut rename_apply): (Option<String>, Option<String>, Option<(String, String)>) = (None, None, None);
        // 그룹 접기 상태(영속) — 현재 접힌 그룹 + 이번 프레임 토글 요청.
        let collapsed = self.config.appearance.collapsed_groups.clone();
        let pinned = self.config.appearance.pinned_sessions.clone();
        let notes = self.config.appearance.session_notes.clone();
        let cues = self.config.appearance.symbol_cues;
        // 연결중 표시(🟢)·마지막 접속 시간 — "세션 관리" 메뉴와 동일 정보(완전 통합).
        //
        // 2026-09-10까지 여기는 **SSH 만** 봤다. 직렬을 더한 뒤에도 안 고쳐서, 직렬·로컬
        // 세션은 열려 있어도 사이드바가 꺼진 것으로 보여 줬다 — 그래서 같은 데를 또 연다.
        // 이제 종류를 가리지 않고 `same_target` 이 판정한다(새 종류가 생기면 컴파일러가 짚는다).
        let active: Vec<(SessionKind, nabi_types::PaneId)> =
            self.pane_origins.iter().map(|(p, k)| (k.clone(), *p)).collect();
        let last_conn = self.config.terminal.last_connected.clone();
        let now = chrono::Local::now().timestamp();
        // 일괄 확인 결과를 세션 이름 기준으로 한 번만 펴 둔다(행마다 잠그지 않게).
        let reach_map: std::collections::HashMap<String, crate::reachall::Reach> = self
            .reach_all
            .lock()
            .ok()
            .map(|m| {
                self.sessions
                    .sessions
                    .iter()
                    .filter_map(|s| match &s.kind {
                        SessionKind::Ssh { host, port, .. } => {
                            m.get(&(host.clone(), *port)).map(|r| (s.name.clone(), *r))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut toggle_group: Option<String> = None;
        egui::Panel::left("sessions_sidebar")
            .default_size(200.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 선택 모드: 켜면 클릭이 '연결'이 아니라 '선택'이 된다(여러 개 고르기).
                    let pm = self.sidebar_pick_mode;
                    if ui.selectable_label(pm, tr(lang, "bulk.pickmode")).on_hover_text(tr(lang, "bulk.pickmode.hint")).clicked() {
                        self.sidebar_pick_mode = !pm;
                        if pm { self.sidebar_marked.clear(); } // 끄면 선택도 비운다.
                    }
                });
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
                // 표식 칩 — 누르면 거르기 칸에 그 낱말이 들어간다. 다시 누르면 지워진다.
                // 새 거르기 통로를 만들지 않는다: 칸 하나가 이름·호스트·표식을 다 본다.
                ui.horizontal_wrapped(|ui| {
                    for t in [
                        nabi_session::SessionTag::Prod,
                        nabi_session::SessionTag::Staging,
                        nabi_session::SessionTag::Dev,
                    ] {
                        let w = t.word();
                        let on = self.sidebar_filter.split_whitespace().any(|x| x == w);
                        let (r8, g8, b8) = t.rgb();
                        let txt = egui::RichText::new(tr(lang, t.key()))
                            .small()
                            .color(egui::Color32::from_rgb(r8, g8, b8));
                        if ui.selectable_label(on, txt).clicked() {
                            let mut words: Vec<String> = self
                                .sidebar_filter
                                .split_whitespace()
                                .map(str::to_string)
                                .filter(|x| x != w)
                                .collect();
                            if !on {
                                words.push(w.to_string());
                            }
                            self.sidebar_filter = words.join(" ");
                        }
                    }
                });
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
                let marked = self.sidebar_marked.clone();
                let fails = self.last_fail.clone();
                let mut click_out: Option<(String, bool, bool)> = None;
                let mut drag_row = |ui: &mut egui::Ui, s: &SavedSession, sel: Option<&str>, ns: &mut Option<String>| -> Option<MenuAction> {
                    // 이미 붙어 있는 pane 이 있으면 그 자리를 들고 온다(눌러서 갈 수 있게).
                    let live_pane = active.iter().find(|(k, _)| k.same_target(&s.kind)).map(|(_, p)| *p);
                    let live = live_pane.is_some();
                    let last = last_conn.get(&s.name).copied();
                    let reach = reach_map.get(&s.name).copied();
                    // 실패는 접속 정보로 찾는다 — 이름은 바뀌어도 접속 정보는 그대로다.
                    let fail = fails.get(&s.kind).cloned();
                    // 드래그 소스는 side_row 내부에서 이름 라벨에만 적용 — 우측 아이콘 클릭이 드래그에 가로채이지 않게.
                    side_row(ui, lang, s, sel, ns, &all_folders, &notes, RowState { live, live_pane, reach, fail, cues }, last, now, marked.contains(&s.name), &mut click_out, menu_row.as_deref() == Some(s.name.as_str()), &mut menu_now)
                };
                // 선택 막대: 몇 개 골랐는지 + 한 번에 연결 / 선택 해제.
                if !self.sidebar_marked.is_empty() {
                    let n = self.sidebar_marked.len();
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        let go = egui::Button::new(
                            egui::RichText::new(format!("\u{25b6} {} ({n})", tr(lang, "bulk.connect"))).color(egui::Color32::WHITE),
                        )
                        .fill(crate::theme_ui::OK);
                        if ui.add(go).clicked() { connect_marked = true; }
                        if ui.button(tr(lang, "bulk.clear")).clicked() { self.sidebar_marked.clear(); }
                    });
                }
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
                    let (_, drop) = ui.dnd_drop_zone::<String, _>(egui::Frame::NONE, |ui| {
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
                        let (_, drop) = ui.dnd_drop_zone::<String, _>(egui::Frame::NONE, |ui| {
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
                                if ui.button(tr(lang, "sessions.connectall")).clicked() { action = Some(MenuAction::ConnectFolder(f.to_string())); ui.close(); }
                                if ui.button(tr(lang, "sessions.renamegroup")).clicked() { start_rename = Some(f.to_string()); ui.close(); }
                                if ui.button(tr(lang, "sessions.ungroupall")).clicked() { ungroup_folder = Some(f.to_string()); ui.close(); }
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
                // 다중 선택 처리: 범위(Shift)는 **보이는 순서**를 아는 여기서 판정한다.
                if let Some((name, ctrl, shift)) = click_out {
                    let order: Vec<String> = vis.iter().map(|s| s.name.clone()).collect();
                    // 선택 모드가 켜져 있으면 평클릭도 '선택'으로 친다 — Ctrl을 모르는 사용자와
                    // 트랙패드 환경을 위해 눈에 보이는 토글을 둔다(Ctrl/Shift는 그대로 동작).
                    let pick_mode = self.sidebar_pick_mode;
                    let picked = crate::sidebarsel::apply_click(
                        &mut self.sidebar_marked, &mut self.sidebar_anchor, &order, &name, ctrl || pick_mode, shift,
                    );
                    if let crate::sidebarsel::RowClick::Connect(n) = picked {
                        if let Some(s) = saved.iter().find(|s| s.name == n) {
                            action = Some(MenuAction::ConnectSaved((*s).clone()));
                        }
                    }
                }
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
        if let Some(g) = toggle_group {
            let v = &mut self.config.appearance.collapsed_groups;
            match v.iter().position(|x| x == &g) {
                Some(i) => drop(v.remove(i)),
                None => v.push(g),
            }
            self.save_config();
        }
        self.sidebar_menu_row = menu_now; // 다음 프레임에 그 행 아이콘을 유지한다.
        if connect_marked {
            let names = self.sidebar_marked.clone();
            self.bulk_connect(&names); // 자격증명 없는 항목이 섞였으면 확인 창을 띄운다.
            self.sidebar_marked.clear();
        }
        if let Some(a) = action { self.apply(ctx, a); }
    }

    /// 그룹 이름을 바꾼다(그 folder의 모든 세션 일괄; 새 이름이 비면 그룹 해제). 저장.
    /// 실제 일괄 조작은 nabi-session::groups 순수 함수(SSOT — 세션 메뉴와 공유).
    pub(crate) fn rename_folder(&mut self, old: &str, new: &str) {
        let n = if new.trim().is_empty() {
            nabi_session::groups::disband_group(&mut self.sessions.sessions, old)
        } else {
            nabi_session::groups::rename_group(&mut self.sessions.sessions, old, new)
        };
        if n > 0 { self.save_sessions(); }
    }

    /// 세션의 표식(운영/개발…)을 바꾸고 저장한다.
    pub(crate) fn set_session_tag(&mut self, name: &str, tag: nabi_session::SessionTag) {
        if let Some(s) = self.sessions.sessions.iter_mut().find(|s| s.name == name) {
            s.tag = tag;
            self.save_sessions();
        }
    }

    /// 세션의 그룹(folder)을 바꾸고 저장한다(사이드바 DnD·우클릭 그룹 이동).
    pub(crate) fn set_session_folder(&mut self, name: &str, folder: Option<String>) {
        if let Some(s) = self.sessions.sessions.iter_mut().find(|s| s.name == name) {
            s.folder = folder;
            self.save_sessions();
        }
    }
}
