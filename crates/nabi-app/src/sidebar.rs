//! 왼쪽 세션 사이드바(MobaXterm식) — 저장 세션 트리: 폴더 그룹·필터·더블클릭 연결.

use crate::app::NabiApp;
use crate::menu::MenuAction;
use nabi_i18n::{tr, Lang};
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
        // 연결중 표시(🟢)·마지막 접속 시간 — "세션 관리" 메뉴와 동일 정보(완전 통합).
        let active: std::collections::HashSet<String> = self.pane_origins.values().filter_map(|k| match k {
            SessionKind::Ssh { host, user, port, .. } => Some(format!("{user}@{host}:{port}")), _ => None }).collect();
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
                    let live = matches!(&s.kind, SessionKind::Ssh { host, user, port, .. } if active.contains(&format!("{user}@{host}:{port}")));
                    let last = last_conn.get(&s.name).copied();
                    let reach = reach_map.get(&s.name).copied();
                    // 실패는 접속 정보로 찾는다 — 이름은 바뀌어도 접속 정보는 그대로다.
                    let fail = fails.get(&s.kind).cloned();
                    // 드래그 소스는 side_row 내부에서 이름 라벨에만 적용 — 우측 아이콘 클릭이 드래그에 가로채이지 않게.
                    side_row(ui, lang, s, sel, ns, &all_folders, &notes, RowState { live, reach, fail }, last, now, marked.contains(&s.name), &mut click_out, menu_row.as_deref() == Some(s.name.as_str()), &mut menu_now)
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
            let _ = nabi_config::save(&self.config_path, &self.config);
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

/// 한 줄의 살아 있는 상태 — 인자를 하나 더 늘리는 대신 묶었다(이 함수는 이미 인자가 많다).
#[derive(Clone, Default)]
pub(crate) struct RowState {
    /// 지금 연결돼 있는가.
    pub live: bool,
    /// 마지막 일괄 확인 결과(안 훑었으면 None).
    pub reach: Option<crate::reachall::Reach>,
    /// 마지막 연결 실패(성공하면 지워진다).
    pub fail: Option<crate::lastfail::LastFail>,
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
    st: RowState,
    last: Option<i64>,
    now: i64,
    marked: bool,
    click_out: &mut Option<(String, bool, bool)>,
    // 직전 프레임에 이 행의 ⋯ 메뉴가 열려 있었는가.
    menu_was_open: bool,
    // 이번 프레임에 이 행의 ⋯ 메뉴가 열려 있으면 이름을 담는다.
    menu_open_out: &mut Option<String>,
) -> Option<MenuAction> {
    let mut action = None;
    let is_ssh = matches!(s.kind, SessionKind::Ssh { .. }) && !s.is_ftp;
    let selected = cur_sel == Some(s.name.as_str()) || marked;
    // 행 전체 사각형을 **먼저** 잡는다. 배경을 이름 영역에만 칠하면 강조 막대가 오른쪽
    // 아이콘 자리에서 뚝 끊겨 지저분해 보이고, 호버 판정도 이름 위에서만 되어
    // 아이콘 쪽으로 마우스를 옮기면 강조가 꺼진다.
    let row_h = (ui.spacing().interact_size.y + 4.0).max(22.0);
    let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), row_h));
    let row_hot = ui.rect_contains_pointer(full);
    // 동작 아이콘은 **가리키거나 선택했을 때만** 보여 준다. 늘 띄워 두면 세션 10개에
    // 아이콘이 40개라 목록이 아이콘 밭이 된다(오클릭 위험도 있다 — 특히 ✕ 삭제).
    // 평소엔 이름만 보이고, 우클릭 메뉴는 행 어디서나 그대로 열린다.
    // ⋯ 메뉴가 열려 있는 동안에는 아이콘을 계속 그린다. 예전엔 행에서 마우스가 벗어나면
    // 버튼 자체가 사라졌고, **버튼에 매인 메뉴도 같이 닫혔다** — 항목을 고르려고 아래로
    // 내려가는 순간 닫혀 쓸 수가 없었다(사용자 보고 2026-08-21). 우클릭 메뉴는 행 응답에
    // 매여 있어 같은 문제가 없었다.
    let show_icons = row_hot || selected || menu_was_open;
    let rounding = egui::CornerRadius::same(4);
    if selected {
        ui.painter().rect_filled(full, rounding, ui.visuals().selection.bg_fill);
    } else if row_hot {
        ui.painter().rect_filled(full, rounding, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let resp = ui.horizontal(|ui| {
        // 아이콘 자리는 **행마다 같은 폭**으로 예약한다. 예전엔 SSH 여부로 90/68을 갈라서
        // 종류가 섞이면 이름 끝나는 위치가 22px씩 어긋나 목록이 들쭉날쭉해 보였다.
        const ICON_W: f32 = 92.0;
        let bw = (ui.available_width() - ICON_W).max(48.0);
        let (rect, r) = ui.allocate_exact_size(egui::vec2(bw, row_h), egui::Sense::click_and_drag());
        let vis = ui.style().interact_selectable(&r, selected);
        let font = egui::TextStyle::Button.resolve(ui.style());
        // 연결 중이면 종류 아이콘을 강조색(초록)으로 — 🟢 점 대신 선두 아이콘 색으로 표시.
        // 표식 띠(운영/스테이징/개발) — 행 왼쪽 가장자리. 목록을 훑을 때 이름보다 먼저 보인다.
        if s.tag != nabi_session::SessionTag::None {
            let (r8, g8, b8) = s.tag.rgb();
            let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
            ui.painter().rect_filled(bar, egui::CornerRadius::ZERO, egui::Color32::from_rgb(r8, g8, b8));
        }
        let kcolor = if st.live { crate::theme_ui::OK } else { crate::theme_ui::session_color(s.is_ftp, is_ssh) };
        ui.painter().text(egui::pos2(rect.left() + 5.0, rect.center().y), egui::Align2::LEFT_CENTER, kind_icon(s), font.clone(), kcolor);
        // 일괄 확인 결과 — 이름 앞에 작은 점. 연결돼 있으면 이미 초록 아이콘이 있으므로
        // 겹쳐 그리지 않는다.
        if let (Some(rc), false) = (st.reach, st.live) {
            ui.painter().text(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                rc.mark(),
                egui::FontId::proportional(9.0),
                rc.color(),
            );
        }
        // 마지막 연결 실패 — 이름 앞 경고 표시. 연결 중이면 이미 지워졌으므로 뜨지 않는다.
        let mut warn_rect = None;
        if let (Some(f), false) = (st.fail.as_ref(), st.live) {
            let pos = egui::pos2(rect.left() + 16.0, rect.center().y);
            let g = ui.painter().text(pos, egui::Align2::LEFT_CENTER, "\u{26a0}", egui::FontId::proportional(10.0), crate::theme_ui::ERR);
            warn_rect = Some((g, crate::lastfail::detail(lang, f)));
        }
        // 이름(+메모 📝) — 폭 넘치면 … 말줄임.
        let note = if notes.get(&s.name).is_some_and(|n| !n.is_empty()) { " \u{1f4dd}" } else { "" };
        let mut job = egui::text::LayoutJob::simple_singleline(format!("{}{note}", s.name), font, vis.text_color());
        job.wrap = egui::text::TextWrapping { max_width: (rect.width() - 26.0).max(16.0), max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, vis.text_color());
        // 왜 실패했는지는 **가리켰을 때** 보여 준다. 목록에 늘 펼쳐 두면 이름이 밀린다.
        if let Some((wr, tip)) = warn_rect {
            // 경고 글리프 자리에만 감지 영역을 둔다 — 행 전체에 붙이면 이름 위에서도 떠서
            // 목록을 훑는 내내 말풍선이 따라다닌다.
            let id = egui::Id::new(("failtip", &s.name));
            ui.interact(wr, id, egui::Sense::hover()).on_hover_text(tip);
        }
        // 드래그(길게 눌러 이동) 페이로드 + 고스트(커서 옆에 이름).
        if r.drag_started() { r.dnd_set_drag_payload(s.name.clone()); }
        if r.dragged() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                egui::Area::new(egui::Id::new(("sdghost", &s.name))).order(egui::Order::Tooltip).fixed_pos(p + egui::vec2(12.0, 4.0))
                    .show(ui.ctx(), |ui| { egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(format!("\u{1f5a5} {}", s.name))); });
            }
        }
        // 인라인 동작 아이콘(우측): ⋯더보기 · 🖧SFTP(SSH) · ✎편집 · ✕삭제.
        //
        // 간격이 1px이었다. 버튼의 둥근 호버 배경은 글자보다 패딩만큼 넓어서, 한 아이콘에
        // 마우스를 올리면 그 배경이 **옆 아이콘 자리까지 겹쳐** 보였다. 글자 간격이 아니라
        // 배경끼리 부딪히지 않을 만큼 띄워야 한다.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if !show_icons {
                return; // 가리키지 않은 행은 이름만 — 자리는 위에서 이미 비워 뒀다.
            }
            let icon = |ui: &mut egui::Ui, g: &str, key: &str| ui.small_button(g).on_hover_text(tr(lang, key)).clicked();
            if icon(ui, "\u{2715}", "sessions.delete") { action = Some(MenuAction::DeleteSession(s.name.clone())); }
            if icon(ui, "\u{270e}", "sessions.edit") { action = Some(MenuAction::EditSession(s.clone())); }
            if is_ssh && icon(ui, "\u{1f5a7}", "sessions.opensftp") { action = Some(MenuAction::OpenSftp(s.clone())); }
            ui.menu_button("\u{22ef}", |ui| {
                *menu_open_out = Some(s.name.clone()); // 열려 있는 동안 아이콘을 유지시킨다.
                if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
            });
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
        // Ctrl/Shift는 '선택'이고 평클릭은 '연결'이다. 범위 선택은 보이는 순서를 아는
        // 호출부에서 판정하므로 여기서는 수정자만 실어 올려보낸다.
        let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
        *click_out = Some((s.name.clone(), ctrl, shift));
        if !ctrl && !shift {
            *new_sel = Some(s.name.clone());
        }
    }
    // 우클릭=더보기(라인 어디서나). 라인 전폭 영역이라 이름 어디서 눌러도 동일 메뉴.
    resp.context_menu(|ui| {
        if let Some(a) = crate::sessionctx::session_menu_items(ui, s, lang, folders) { action = Some(a); }
    });
    action
}

